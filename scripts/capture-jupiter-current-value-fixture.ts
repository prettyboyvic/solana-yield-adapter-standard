/**
 * Capture a raw mainnet Jupiter Pool + JLP mint snapshot and prove the JLP fair
 * value formula against the deployed program's Anchor IDL decoder.
 *
 *   RPC_URL=<mainnet-rpc> npm run jupiter:current-value:capture
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";
import { BorshAccountsCoder } from "@coral-xyz/anchor";
import { Connection, PublicKey } from "@solana/web3.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const derived = JSON.parse(
  fs.readFileSync(path.join(repoRoot, "docs/jupiter-derived-accounts.json"), "utf8"),
);
const rpc = process.env.RPC_URL ?? "https://api.mainnet-beta.solana.com";
const connection = new Connection(rpc, "finalized");
const program = new PublicKey(derived.program);
const pool = new PublicKey(derived.pool.address);
const jlpMint = new PublicKey(derived.receiptMint);

async function main() {
  const { idl, idlAddress } = await fetchOnChainIdl(connection, program);
  const coder = new BorshAccountsCoder(idl);
  const snapshot = await connection.getMultipleAccountsInfoAndContext([pool, jlpMint], "finalized");
  const [poolInfo, mintInfo] = snapshot.value;
  if (!poolInfo || !poolInfo.owner.equals(program)) throw new Error("Jupiter Pool missing/wrong owner");
  if (!mintInfo || mintInfo.owner.toBase58() !== "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA") {
    throw new Error("JLP mint missing/wrong owner");
  }

  const poolState: any = coder.decode("Pool", poolInfo.data);
  const aumUsd = BigInt(poolState.aumUsd.toString(10));
  const jlpMintSupply = mintInfo.data.readBigUInt64LE(36);
  const sampleJlpAmount = 1_000_000n;
  const expectedAssets = (sampleJlpAmount * aumUsd) / jlpMintSupply;
  const slot = snapshot.context.slot;
  const outputPath = path.join(repoRoot, `tests/fixtures/jupiter-current-value-${slot}.json`);

  const fixture = {
    schema: "jupiter-current-value-fixture/v1",
    note:
      "Raw mainnet Pool + JLP mint snapshot. sampleJlpAmount is a deterministic one-JLP amount used to prove the decoder/formula; it is not live fork evidence.",
    rpc,
    commitment: "finalized",
    slot,
    fetchedAt: new Date().toISOString(),
    jupiterPerps: {
      program: program.toBase58(),
      onChainIdl: idlAddress.toBase58(),
      idlName: idl.name,
      idlVersion: idl.version,
    },
    accounts: {
      pool: accountJson(pool, poolInfo),
      jlpMint: accountJson(jlpMint, mintInfo),
    },
    idlOracle: {
      pool: pool.toBase58(),
      jlpMint: jlpMint.toBase58(),
      aumUsd: aumUsd.toString(),
      aumUsdRefreshedAtSlot: poolState.aumUsdRefreshedAtSlot.toString(10),
      jlpMintSupply: jlpMintSupply.toString(),
      sampleJlpAmount: sampleJlpAmount.toString(),
      expectedAssets: expectedAssets.toString(),
      formula: "floor(sampleJlpAmount * pool.aumUsd / jlpMintSupply)",
    },
  };

  fs.writeFileSync(outputPath, JSON.stringify(fixture, null, 2) + "\n");
  console.log("wrote", path.relative(process.cwd(), outputPath));
  console.log(
    `slot=${slot} aumUsd=${aumUsd} jlpMintSupply=${jlpMintSupply} expectedAssetsForOneJlp=${expectedAssets}`,
  );
}

async function fetchOnChainIdl(connection: Connection, program: PublicKey) {
  const base = PublicKey.findProgramAddressSync([], program)[0];
  const idlAddress = await PublicKey.createWithSeed(base, "anchor:idl", program);
  const account = await connection.getAccountInfo(idlAddress, "finalized");
  if (!account || !account.owner.equals(program)) throw new Error("on-chain IDL missing/wrong owner");
  const compressedLength = account.data.readUInt32LE(40);
  const json = inflateSync(account.data.subarray(44, 44 + compressedLength)).toString("utf8");
  return { idl: JSON.parse(json), idlAddress };
}

function accountJson(pubkey: PublicKey, info: any) {
  return {
    pubkey: pubkey.toBase58(),
    owner: info.owner.toBase58(),
    lamports: info.lamports,
    executable: info.executable,
    dataLength: info.data.length,
    dataBase64: info.data.toString("base64"),
  };
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
