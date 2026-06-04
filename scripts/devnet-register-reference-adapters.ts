import * as fs from "node:fs";
import { createHash } from "node:crypto";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  DERIVE,
  DISPATCHER_PROGRAM_ID,
  PENDING,
  REFERENCE_ADAPTERS,
  REFERENCE_ADAPTER_PROGRAM_ID,
  bytesToHex,
} from "../packages/sdk/src/index.js";

const RPC_URL = process.env.DEVNET_RPC_URL ?? "https://api.devnet.solana.com";
const KEYPAIR_PATH = process.env.DEVNET_PAYER_KEYPAIR ?? "target/deploy/devnet-payer.json";
const REGISTRY_METADATA_URI =
  process.env.REGISTRY_METADATA_URI ?? "ipfs://solana-yield-adapters/registry.json";
const MAX_ADAPTERS = 5;
const DEFAULT_PUBKEY = PublicKey.default.toBase58();

type RegistryRecord = {
  label: string;
  adapterIdHex: string;
  programId: string;
  protocol: number;
  underlyingMint: string;
  receiptMint: string;
  authority: string;
  capabilities: number;
  riskTier: number;
  active: boolean;
  metadataUri: string;
  metadataHash: string;
};

type RegistrySnapshot = {
  address: string;
  governance: string;
  bump: number;
  version: number;
  adapterCount: number;
  maxAdapters: number;
  metadataUri: string;
  records: RegistryRecord[];
};

function anchorDiscriminator(name: string): Buffer {
  return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

function accountDiscriminator(name: string): Buffer {
  return createHash("sha256").update(`account:${name}`).digest().subarray(0, 8);
}

function writeU16(value: number): Buffer {
  const out = Buffer.alloc(2);
  out.writeUInt16LE(value);
  return out;
}

function writeString(value: string): Buffer {
  const bytes = Buffer.from(value, "utf8");
  const len = Buffer.alloc(4);
  len.writeUInt32LE(bytes.length);
  return Buffer.concat([len, bytes]);
}

function pubkeyBytes(value: string): Buffer {
  return Buffer.from(new PublicKey(value).toBytes());
}

function metadataHash(value: string): Buffer {
  return createHash("sha256").update(value).digest();
}

function readKeypair(path: string): Keypair {
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(path, "utf8"))));
}

function registryPda(): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("registry")],
    new PublicKey(DISPATCHER_PROGRAM_ID),
  )[0];
}

function initRegistryIx(registry: PublicKey, payer: PublicKey): TransactionInstruction {
  const data = Buffer.concat([
    anchorDiscriminator("initialize_registry"),
    writeU16(MAX_ADAPTERS),
    writeString(REGISTRY_METADATA_URI),
  ]);
  return new TransactionInstruction({
    programId: new PublicKey(DISPATCHER_PROGRAM_ID),
    keys: [
      { pubkey: registry, isSigner: false, isWritable: true },
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: payer, isSigner: true, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

function receiptMintForRegistry(receiptMint: string): string {
  if (receiptMint === DERIVE || receiptMint === PENDING) return DEFAULT_PUBKEY;
  return receiptMint;
}

function registerAdapterIx(
  registry: PublicKey,
  governance: PublicKey,
  adapter: (typeof REFERENCE_ADAPTERS)[number],
): TransactionInstruction {
  const metadataUri = adapter.metadataUri;
  const receiptMint = receiptMintForRegistry(adapter.receiptMint);
  const data = Buffer.concat([
    anchorDiscriminator("register_adapter"),
    Buffer.from(adapter.adapterId),
    Buffer.from([adapter.protocol]),
    pubkeyBytes(adapter.underlyingMint),
    pubkeyBytes(receiptMint),
    pubkeyBytes(governance.toBase58()),
    writeU16(adapter.capabilities),
    Buffer.from([adapter.riskTier]),
    writeString(metadataUri),
    metadataHash(metadataUri),
  ]);
  return new TransactionInstruction({
    programId: new PublicKey(DISPATCHER_PROGRAM_ID),
    keys: [
      { pubkey: registry, isSigner: false, isWritable: true },
      { pubkey: governance, isSigner: true, isWritable: false },
      { pubkey: new PublicKey(REFERENCE_ADAPTER_PROGRAM_ID), isSigner: false, isWritable: false },
    ],
    data,
  });
}

function readPubkey(data: Buffer, offset: { value: number }): string {
  const out = new PublicKey(data.subarray(offset.value, offset.value + 32)).toBase58();
  offset.value += 32;
  return out;
}

function readString(data: Buffer, offset: { value: number }): string {
  const len = data.readUInt32LE(offset.value);
  offset.value += 4;
  const out = data.subarray(offset.value, offset.value + len).toString("utf8");
  offset.value += len;
  return out;
}

function labelForAdapterId(adapterIdHex: string): string {
  return (
    REFERENCE_ADAPTERS.find((adapter) => bytesToHex(adapter.adapterId) === adapterIdHex)?.label ??
    "<unknown>"
  );
}

async function readRegistry(
  connection: Connection,
  registry: PublicKey,
): Promise<RegistrySnapshot | null> {
  const account = await connection.getAccountInfo(registry, "confirmed");
  if (!account) return null;
  if (!account.owner.equals(new PublicKey(DISPATCHER_PROGRAM_ID))) {
    throw new Error(`registry owner mismatch: ${account.owner.toBase58()}`);
  }
  const expected = accountDiscriminator("Registry");
  const actual = account.data.subarray(0, 8);
  if (!Buffer.from(actual).equals(expected)) {
    throw new Error(
      `registry discriminator mismatch: got ${Buffer.from(actual).toString(
        "hex",
      )}, expected ${expected.toString("hex")}`,
    );
  }

  const offset = { value: 8 };
  const governance = readPubkey(account.data, offset);
  const bump = account.data[offset.value];
  offset.value += 1;
  const version = account.data.readUInt16LE(offset.value);
  offset.value += 2;
  const adapterCount = account.data.readUInt16LE(offset.value);
  offset.value += 2;
  const maxAdapters = account.data.readUInt16LE(offset.value);
  offset.value += 2;
  const metadataUri = readString(account.data, offset);
  const recordCount = account.data.readUInt32LE(offset.value);
  offset.value += 4;

  const records: RegistryRecord[] = [];
  for (let i = 0; i < recordCount; i += 1) {
    const adapterIdHex = account.data.subarray(offset.value, offset.value + 32).toString("hex");
    offset.value += 32;
    const programId = readPubkey(account.data, offset);
    const protocol = account.data[offset.value];
    offset.value += 1;
    const underlyingMint = readPubkey(account.data, offset);
    const receiptMint = readPubkey(account.data, offset);
    const authority = readPubkey(account.data, offset);
    const capabilities = account.data.readUInt16LE(offset.value);
    offset.value += 2;
    const riskTier = account.data[offset.value];
    offset.value += 1;
    const active = account.data[offset.value] !== 0;
    offset.value += 1;
    const recordMetadataUri = readString(account.data, offset);
    const recordMetadataHash = account.data
      .subarray(offset.value, offset.value + 32)
      .toString("hex");
    offset.value += 32;

    records.push({
      label: labelForAdapterId(adapterIdHex),
      adapterIdHex,
      programId,
      protocol,
      underlyingMint,
      receiptMint,
      authority,
      capabilities,
      riskTier,
      active,
      metadataUri: recordMetadataUri,
      metadataHash: recordMetadataHash,
    });
  }

  if (adapterCount !== recordCount) {
    throw new Error(`registry adapter_count=${adapterCount} but vector has ${recordCount}`);
  }

  return {
    address: registry.toBase58(),
    governance,
    bump,
    version,
    adapterCount,
    maxAdapters,
    metadataUri,
    records,
  };
}

function assertFiveRegistered(snapshot: RegistrySnapshot): void {
  const expectedLabels = REFERENCE_ADAPTERS.map((adapter) => adapter.label);
  const missing = expectedLabels.filter(
    (label) => !snapshot.records.some((record) => record.label === label),
  );
  if (missing.length > 0) {
    throw new Error(`missing registered adapters: ${missing.join(", ")}`);
  }

  for (const adapter of REFERENCE_ADAPTERS) {
    const record = snapshot.records.find((candidate) => candidate.label === adapter.label);
    if (!record) throw new Error(`missing ${adapter.label}`);
    const expectedReceipt = receiptMintForRegistry(adapter.receiptMint);
    if (record.programId !== REFERENCE_ADAPTER_PROGRAM_ID) {
      throw new Error(`${adapter.label} program mismatch: ${record.programId}`);
    }
    if (record.protocol !== adapter.protocol) {
      throw new Error(`${adapter.label} protocol mismatch: ${record.protocol}`);
    }
    if (record.underlyingMint !== adapter.underlyingMint) {
      throw new Error(`${adapter.label} underlying mismatch: ${record.underlyingMint}`);
    }
    if (record.receiptMint !== expectedReceipt) {
      throw new Error(`${adapter.label} receipt mismatch: ${record.receiptMint}`);
    }
    if (record.capabilities !== adapter.capabilities) {
      throw new Error(`${adapter.label} capabilities mismatch: ${record.capabilities}`);
    }
    if (record.riskTier !== adapter.riskTier) {
      throw new Error(`${adapter.label} risk tier mismatch: ${record.riskTier}`);
    }
    if (!record.active) {
      throw new Error(`${adapter.label} is not active`);
    }
    if (record.metadataUri !== adapter.metadataUri) {
      throw new Error(`${adapter.label} metadata URI mismatch: ${record.metadataUri}`);
    }
    if (record.metadataHash !== metadataHash(adapter.metadataUri).toString("hex")) {
      throw new Error(`${adapter.label} metadata hash mismatch: ${record.metadataHash}`);
    }
  }
}

async function sendOne(
  connection: Connection,
  payer: Keypair,
  instruction: TransactionInstruction,
): Promise<string> {
  const tx = new Transaction().add(instruction);
  return sendAndConfirmTransaction(connection, tx, [payer], {
    commitment: "confirmed",
    skipPreflight: false,
  });
}

async function main(): Promise<void> {
  const connection = new Connection(RPC_URL, "confirmed");
  const payer = readKeypair(KEYPAIR_PATH);
  const registry = registryPda();

  const dispatcher = await connection.getAccountInfo(new PublicKey(DISPATCHER_PROGRAM_ID));
  if (!dispatcher?.executable) throw new Error("dispatcher program is not executable on devnet");
  const adapterProgram = await connection.getAccountInfo(new PublicKey(REFERENCE_ADAPTER_PROGRAM_ID));
  if (!adapterProgram?.executable) {
    throw new Error("reference adapter program is not executable on devnet");
  }

  const signatures: { action: string; signature: string }[] = [];
  let snapshot = await readRegistry(connection, registry);
  if (!snapshot) {
    signatures.push({
      action: "initialize_registry",
      signature: await sendOne(connection, payer, initRegistryIx(registry, payer.publicKey)),
    });
    snapshot = await readRegistry(connection, registry);
  }
  if (!snapshot) throw new Error("registry did not initialize");
  if (snapshot.governance !== payer.publicKey.toBase58()) {
    throw new Error(
      `registry governance ${snapshot.governance} does not match payer ${payer.publicKey.toBase58()}`,
    );
  }

  for (const adapter of REFERENCE_ADAPTERS) {
    const label = adapter.label;
    if (snapshot.records.some((record) => record.label === label)) continue;
    signatures.push({
      action: `register_adapter:${label}`,
      signature: await sendOne(
        connection,
        payer,
        registerAdapterIx(registry, payer.publicKey, adapter),
      ),
    });
    snapshot = (await readRegistry(connection, registry)) ?? snapshot;
  }

  const verified = await readRegistry(connection, registry);
  if (!verified) throw new Error("registry missing after registration");
  assertFiveRegistered(verified);

  console.log(
    JSON.stringify(
      {
        rpcUrl: RPC_URL,
        payer: payer.publicKey.toBase58(),
        dispatcherProgram: DISPATCHER_PROGRAM_ID,
        referenceAdapterProgram: REFERENCE_ADAPTER_PROGRAM_ID,
        registry: verified,
        signatures,
        receiptPlaceholder:
          "Adapters with derived/no static receipt mint are stored as the default pubkey.",
      },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
