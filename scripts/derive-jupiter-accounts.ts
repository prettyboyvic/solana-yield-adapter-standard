/**
 * Derive + verify the Jupiter Perps USDC -> JLP account map directly from the
 * deployed program's Anchor IDL and mainnet account state.
 *
 *   RPC_URL=<mainnet-rpc> npm run jupiter:derive
 *   npm run jupiter:cpi-plan
 */
import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";
import { BorshAccountsCoder } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";

const JUPITER_PERPS_PROGRAM = new PublicKey(
  "PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu",
);
const JLP_MINT = new PublicKey("27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4");
const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const DOVES_PROGRAM = new PublicKey("DoVEsk76QybCEHQGzkvYPWLQu9gzNoZZZt3TPiL597e");
const ASSOCIATED_TOKEN_PROGRAM = new PublicKey(
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
);
const REFERENCE_ADAPTER_PROGRAM = new PublicKey(
  "BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1",
);

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const outPath = path.resolve(__dirname, "../docs/jupiter-derived-accounts.json");

type OldAnchorIdl = {
  name: string;
  version: string;
  instructions: Array<{
    name: string;
    accounts: Array<{ name: string; isSigner: boolean; isMut: boolean }>;
    args: Array<{ name: string }>;
  }>;
};

async function main() {
  const rpc = process.env.RPC_URL ?? "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpc, "confirmed");
  const { idl, idlAddress } = await fetchOnChainIdl(connection, JUPITER_PERPS_PROGRAM);
  const coder = new BorshAccountsCoder(idl as any);

  const [perpetuals, perpetualsBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("perpetuals")],
    JUPITER_PERPS_PROGRAM,
  );
  const [transferAuthority, transferAuthorityBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("transfer_authority")],
    JUPITER_PERPS_PROGRAM,
  );
  const [eventAuthority, eventAuthorityBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("__event_authority")],
    JUPITER_PERPS_PROGRAM,
  );

  const perpetualsInfo = await connection.getAccountInfoAndContext(perpetuals, "confirmed");
  assertProgramOwned("perpetuals", perpetualsInfo.value, JUPITER_PERPS_PROGRAM);
  const perpetualsState: any = coder.decode("Perpetuals", perpetualsInfo.value!.data);
  const pools = (perpetualsState.pools as PublicKey[]).map((value) => new PublicKey(value));
  const jlpPools = pools.filter((pool) =>
    derivePda(["lp_token_mint", pool], JUPITER_PERPS_PROGRAM).equals(JLP_MINT),
  );
  if (jlpPools.length !== 1) {
    throw new Error(`expected exactly one JLP pool, found ${jlpPools.length}`);
  }
  const pool = jlpPools[0];

  const poolInfo = await connection.getAccountInfoAndContext(pool, "confirmed");
  assertProgramOwned("pool", poolInfo.value, JUPITER_PERPS_PROGRAM);
  const poolState: any = coder.decode("Pool", poolInfo.value!.data);
  const custodyKeys = (poolState.custodies as PublicKey[]).map((value) => new PublicKey(value));
  const custodyInfos = await connection.getMultipleAccountsInfoAndContext(custodyKeys, "confirmed");
  const custodies = custodyKeys.map((key, index) => {
    const info = custodyInfos.value[index];
    assertProgramOwned(`custody ${key.toBase58()}`, info, JUPITER_PERPS_PROGRAM);
    return { key, state: coder.decode("Custody", info!.data) as any };
  });
  const dovesAgPriceAccounts = custodies.map(({ state }) => new PublicKey(state.dovesAgOracle));
  const dovesAgInfos = await connection.getMultipleAccountsInfoAndContext(
    dovesAgPriceAccounts,
    "confirmed",
  );
  dovesAgPriceAccounts.forEach((key, index) =>
    assertProgramOwned(
      `Doves AG price account ${key.toBase58()}`,
      dovesAgInfos.value[index],
      DOVES_PROGRAM,
    ),
  );
  const aumRemainingAccounts = [
    ...custodyKeys.map((key, index) => ({
      name: `aumCustody${index}`,
      pubkey: key.toBase58(),
      isSigner: false,
      isWritable: false,
    })),
    ...dovesAgPriceAccounts.map((key, index) => ({
      name: `aumDovesAgPriceAccount${index}`,
      pubkey: key.toBase58(),
      isSigner: false,
      isWritable: false,
    })),
  ];
  const usdcCustodies = custodies.filter(({ state }) => new PublicKey(state.mint).equals(USDC_MINT));
  if (usdcCustodies.length !== 1) {
    throw new Error(`expected exactly one USDC custody, found ${usdcCustodies.length}`);
  }
  const usdcCustody = usdcCustodies[0];
  const custodyTokenAccount = new PublicKey(usdcCustody.state.tokenAccount);
  const custodyDovesAgPriceAccount = new PublicKey(usdcCustody.state.dovesAgOracle);
  const custodyPythnetPriceAccount = new PublicKey(usdcCustody.state.oracle.oracleAccount);

  assertEqual(
    "USDC custody PDA",
    usdcCustody.key,
    derivePda(["custody", pool, USDC_MINT], JUPITER_PERPS_PROGRAM),
  );
  assertEqual(
    "USDC custody token PDA",
    custodyTokenAccount,
    derivePda(["custody_token_account", pool, USDC_MINT], JUPITER_PERPS_PROGRAM),
  );
  assertEqual(
    "JLP mint PDA",
    JLP_MINT,
    derivePda(["lp_token_mint", pool], JUPITER_PERPS_PROGRAM),
  );

  const staticKeys = [
    JUPITER_PERPS_PROGRAM,
    JLP_MINT,
    USDC_MINT,
    perpetuals,
    transferAuthority,
    pool,
    usdcCustody.key,
    custodyTokenAccount,
    custodyDovesAgPriceAccount,
    custodyPythnetPriceAccount,
    eventAuthority,
  ];
  const staticInfos = await connection.getMultipleAccountsInfoAndContext(staticKeys, "confirmed");
  const accountInfo = Object.fromEntries(
    staticKeys.map((key, index) => [
      key.toBase58(),
      staticInfos.value[index]
        ? {
            owner: staticInfos.value[index]!.owner.toBase58(),
            executable: staticInfos.value[index]!.executable,
            dataLength: staticInfos.value[index]!.data.length,
          }
        : null,
    ]),
  );

  assertTokenOwned("JLP mint", staticInfos.value[1]);
  assertTokenOwned("USDC mint", staticInfos.value[2]);
  assertProgramOwned("transfer authority", staticInfos.value[4], JUPITER_PERPS_PROGRAM);
  assertTokenOwned("USDC custody token account", staticInfos.value[7]);
  if (!staticInfos.value[0]?.executable) throw new Error("Jupiter Perps program is not executable");
  if (!staticInfos.value[8]) throw new Error("USDC Doves AG price account is missing");
  if (!staticInfos.value[9]) throw new Error("USDC Pythnet price account is missing");

  const adapterId = createHash("sha256")
    .update("solana-yield-adapter:jupiter-lp")
    .digest();
  const [adapterState, adapterStateBump] = PublicKey.findProgramAddressSync(
    [Buffer.from("adapter"), adapterId],
    REFERENCE_ADAPTER_PROGRAM,
  );
  const adapterUnderlyingVault = associatedTokenAddress(adapterState, USDC_MINT);
  const adapterJlpVault = associatedTokenAddress(adapterState, JLP_MINT);

  const addLiquidity2 = instructionPlan(idl, "addLiquidity2");
  const removeLiquidity2 = instructionPlan(idl, "removeLiquidity2");
  const cloneAccounts = dedupePublicKeys([
    USDC_MINT,
    JLP_MINT,
    perpetuals,
    transferAuthority,
    pool,
    usdcCustody.key,
    custodyTokenAccount,
    custodyDovesAgPriceAccount,
    custodyPythnetPriceAccount,
    ...custodyKeys,
    ...dovesAgPriceAccounts,
  ]);

  const out = {
    schema: "jupiter-derived-accounts/v1",
    source:
      "Jupiter Perps Anchor IDL stored on mainnet + verified mainnet account reads and PDA derivations",
    generatedBy: "scripts/derive-jupiter-accounts.ts",
    observedSlot: Math.max(
      perpetualsInfo.context.slot,
      poolInfo.context.slot,
      custodyInfos.context.slot,
      dovesAgInfos.context.slot,
      staticInfos.context.slot,
    ),
    program: JUPITER_PERPS_PROGRAM.toBase58(),
    onChainIdl: {
      address: idlAddress.toBase58(),
      name: idl.name,
      version: idl.version,
    },
    underlyingMint: USDC_MINT.toBase58(),
    receiptMint: JLP_MINT.toBase58(),
    perpetuals: perpetuals.toBase58(),
    perpetualsBump,
    transferAuthority: transferAuthority.toBase58(),
    transferAuthorityBump,
    eventAuthority: eventAuthority.toBase58(),
    eventAuthorityBump,
    eventAuthorityExistsOnMainnet: staticInfos.value[10] !== null,
    pool: {
      address: pool.toBase58(),
      name: String(poolState.name),
      aumUsd: poolState.aumUsd.toString(10),
      aumUsdRefreshedAtSlot: poolState.aumUsdRefreshedAtSlot.toString(10),
      custodies: custodyKeys.map((key) => key.toBase58()),
      dovesAgPriceAccounts: dovesAgPriceAccounts.map((key) => key.toBase58()),
      aumRemainingAccounts,
    },
    usdcCustody: {
      address: usdcCustody.key.toBase58(),
      tokenAccount: custodyTokenAccount.toBase58(),
      dovesAgPriceAccount: custodyDovesAgPriceAccount.toBase58(),
      pythnetPriceAccount: custodyPythnetPriceAccount.toBase58(),
      decimals: Number(usdcCustody.state.decimals),
      isStable: Boolean(usdcCustody.state.isStable),
    },
    adapterCustody: {
      adapterIdHex: adapterId.toString("hex"),
      adapterState: adapterState.toBase58(),
      adapterStateBump,
      adapterUnderlyingVault: adapterUnderlyingVault.toBase58(),
      adapterJlpVault: adapterJlpVault.toBase58(),
    },
    currentValue: {
      formula: "floor(adapterJlpAmount * pool.aumUsd / jlpMintSupply)",
      unit:
        "pool.aumUsd and JLP mint both use 6 decimals, so the result is USDC lamports (6 decimals)",
      accounts: [pool.toBase58(), JLP_MINT.toBase58(), adapterJlpVault.toBase58()],
    },
    instructions: { addLiquidity2, removeLiquidity2 },
    cloneAccounts: cloneAccounts.map((key) => key.toBase58()),
    accountInfo,
    disclaimer:
      "Account derivation only. This file does not claim that Jupiter CPI or the mainnet-fork roundtrip passes.",
  };

  fs.writeFileSync(outPath, JSON.stringify(out, null, 2) + "\n");
  console.log("wrote", path.relative(process.cwd(), outPath));
  console.log(`pool=${pool.toBase58()} custody=${usdcCustody.key.toBase58()}`);
}

async function fetchOnChainIdl(connection: Connection, program: PublicKey) {
  const base = PublicKey.findProgramAddressSync([], program)[0];
  const idlAddress = await PublicKey.createWithSeed(base, "anchor:idl", program);
  const account = await connection.getAccountInfo(idlAddress, "confirmed");
  assertProgramOwned("on-chain IDL", account, program);
  const compressedLength = account!.data.readUInt32LE(40);
  const json = inflateSync(account!.data.subarray(44, 44 + compressedLength)).toString("utf8");
  return { idl: JSON.parse(json) as OldAnchorIdl, idlAddress };
}

function instructionPlan(idl: OldAnchorIdl, name: string) {
  const instruction = idl.instructions.find((candidate) => candidate.name === name);
  if (!instruction) throw new Error(`on-chain IDL is missing ${name}`);
  return {
    instruction: name,
    discriminator: [...anchorDiscriminator(toSnakeCase(name))],
    argNames: instruction.args.map((arg) => arg.name),
    accounts: instruction.accounts.map((account) => ({
      name: account.name,
      isSigner: account.isSigner,
      isWritable: account.isMut,
    })),
  };
}

function anchorDiscriminator(name: string) {
  return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

function toSnakeCase(value: string) {
  return value.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

function derivePda(seeds: Array<string | PublicKey>, program: PublicKey) {
  return PublicKey.findProgramAddressSync(
    seeds.map((seed) => (typeof seed === "string" ? Buffer.from(seed) : seed.toBuffer())),
    program,
  )[0];
}

function associatedTokenAddress(owner: PublicKey, mint: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [owner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM,
  )[0];
}

function dedupePublicKeys(keys: PublicKey[]) {
  return [...new Map(keys.map((key) => [key.toBase58(), key])).values()];
}

function assertProgramOwned(label: string, info: any, owner: PublicKey) {
  if (!info || !info.owner.equals(owner)) {
    throw new Error(`${label} is missing or has the wrong owner`);
  }
}

function assertTokenOwned(label: string, info: any) {
  if (!info || !info.owner.equals(TOKEN_PROGRAM)) {
    throw new Error(`${label} is missing or is not owned by the SPL Token program`);
  }
}

function assertEqual(label: string, actual: PublicKey, expected: PublicKey) {
  if (!actual.equals(expected)) {
    throw new Error(`${label} mismatch: actual=${actual.toBase58()} expected=${expected.toBase58()}`);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
