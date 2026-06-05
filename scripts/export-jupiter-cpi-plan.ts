/**
 * Regenerate the deterministic Jupiter Perps CPI account plan from the committed
 * on-chain-derived account map.
 *
 *   npm run jupiter:cpi-plan
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { PublicKey } from "@solana/web3.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const derivedPath = path.resolve(__dirname, "../docs/jupiter-derived-accounts.json");
const fixturePath = path.resolve(
  __dirname,
  "../packages/sdk/fixtures/jupiter-cpi-account-plan.json",
);
const d = JSON.parse(fs.readFileSync(derivedPath, "utf8"));

const program = new PublicKey(d.program);
const pool = new PublicKey(d.pool.address);
const usdc = new PublicKey(d.underlyingMint);
const jlp = new PublicKey(d.receiptMint);
assertEqual(
  "perpetuals PDA",
  d.perpetuals,
  PublicKey.findProgramAddressSync([Buffer.from("perpetuals")], program)[0].toBase58(),
);
assertEqual(
  "transfer authority PDA",
  d.transferAuthority,
  PublicKey.findProgramAddressSync([Buffer.from("transfer_authority")], program)[0].toBase58(),
);
assertEqual(
  "JLP mint PDA",
  d.receiptMint,
  PublicKey.findProgramAddressSync([Buffer.from("lp_token_mint"), pool.toBuffer()], program)[0].toBase58(),
);
assertEqual(
  "USDC custody PDA",
  d.usdcCustody.address,
  PublicKey.findProgramAddressSync(
    [Buffer.from("custody"), pool.toBuffer(), usdc.toBuffer()],
    program,
  )[0].toBase58(),
);
assertEqual(
  "USDC custody token PDA",
  d.usdcCustody.tokenAccount,
  PublicKey.findProgramAddressSync(
    [Buffer.from("custody_token_account"), pool.toBuffer(), usdc.toBuffer()],
    program,
  )[0].toBase58(),
);
assertEqual("JLP mint key", jlp.toBase58(), d.receiptMint);

const accountKeys: Record<string, string> = {
  owner: d.adapterCustody.adapterState,
  fundingAccount: d.adapterCustody.adapterUnderlyingVault,
  receivingAccount: d.adapterCustody.adapterUnderlyingVault,
  lpTokenAccount: d.adapterCustody.adapterJlpVault,
  transferAuthority: d.transferAuthority,
  perpetuals: d.perpetuals,
  pool: d.pool.address,
  custody: d.usdcCustody.address,
  custodyDovesPriceAccount: d.usdcCustody.dovesAgPriceAccount,
  custodyPythnetPriceAccount: d.usdcCustody.pythnetPriceAccount,
  custodyTokenAccount: d.usdcCustody.tokenAccount,
  lpTokenMint: d.receiptMint,
  tokenProgram: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
  eventAuthority: d.eventAuthority,
  program: d.program,
};

function resolvePlan(name: "addLiquidity2" | "removeLiquidity2") {
  const plan = d.instructions[name];
  return {
    ...plan,
    accounts: plan.accounts.map((account: any) => {
      const pubkey = accountKeys[account.name];
      if (!pubkey) throw new Error(`missing concrete Jupiter key for ${account.name}`);
      return { ...account, pubkey };
    }),
  };
}

const fixture = {
  schema: "jupiter-cpi-account-plan/v1",
  source:
    "docs/jupiter-derived-accounts.json, derived from the deployed Jupiter Perps Anchor IDL and verified mainnet account state",
  generatedBy: "scripts/export-jupiter-cpi-plan.ts",
  program: d.program,
  underlyingMint: d.underlyingMint,
  receiptMint: d.receiptMint,
  pool: d.pool.address,
  perpetuals: d.perpetuals,
  adapterCustody: d.adapterCustody,
  currentValue: d.currentValue,
  aumRemainingAccounts: d.pool.aumRemainingAccounts,
  cloneAccounts: d.cloneAccounts,
  eventAuthorityExistsOnMainnet: d.eventAuthorityExistsOnMainnet,
  plans: {
    addLiquidity2: resolvePlan("addLiquidity2"),
    removeLiquidity2: resolvePlan("removeLiquidity2"),
  },
  txShape:
    "The adapter transfers user USDC into the state-PDA USDC ATA, invokes addLiquidity2/removeLiquidity2 with the state PDA as owner, appends the pool's five custodies then five Doves AG price accounts for Jupiter's AUM calculation, holds JLP in the state-PDA JLP ATA, and values JLP from pool.aumUsd / JLP mint supply.",
  disclaimer:
    "Canonical account plan only. This fixture does not claim that Jupiter CPI or the mainnet-fork roundtrip passes.",
};

fs.writeFileSync(fixturePath, JSON.stringify(fixture, null, 2) + "\n");
console.log("wrote", path.relative(process.cwd(), fixturePath));

function assertEqual(label: string, actual: string, expected: string) {
  if (actual !== expected) throw new Error(`${label} mismatch: actual=${actual} expected=${expected}`);
}
