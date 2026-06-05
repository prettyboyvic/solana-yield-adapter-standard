#!/usr/bin/env node
import { createRequire } from "node:module";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { Connection, PublicKey } from "@solana/web3.js";

const require = createRequire(import.meta.url);
const marginfi = require("@mrgnlabs/marginfi-client-v2");
const marginfiPackage = require("@mrgnlabs/marginfi-client-v2/package.json");
const marginfiPackageRoot = path.dirname(
  require.resolve("@mrgnlabs/marginfi-client-v2/package.json"),
);
const { BorshCoder } = require(
  require.resolve("@coral-xyz/anchor", { paths: [marginfiPackageRoot] }),
);

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const accountMap = readJson("packages/sdk/fixtures/marginfi-mainnet-fork-account-map.json");
const cpiPlan = readJson("packages/sdk/fixtures/marginfi-cpi-account-plan.json");
const rawFixture = readJson("tests/fixtures/marginfi-current-value-424351480.json");

if (marginfiPackage.version !== "6.4.2") {
  throw new Error(`expected @mrgnlabs/marginfi-client-v2@6.4.2, got ${marginfiPackage.version}`);
}
if (accountMap.evidenceSlot !== rawFixture.slot) {
  throw new Error("fork account-map slot does not match the raw bank fixture");
}

const bankKey = new PublicKey(accountMap.cloneAccounts.usdcBank);
const programId = new PublicKey(accountMap.programs.marginfi);
const coder = new BorshCoder(marginfi.MARGINFI_IDL);
const bankData = Buffer.from(rawFixture.accounts.bank.dataBase64, "base64");
const bank = coder.accounts.decode("Bank", bankData);
const expected = {
  program: rawFixture.marginfiProgramId,
  group: new PublicKey(bank.group).toBase58(),
  mint: new PublicKey(bank.mint).toBase58(),
  oracle: new PublicKey(bank.config.oracle_keys[0]).toBase58(),
  liquidityVault: PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault"), bankKey.toBuffer()],
    programId,
  )[0].toBase58(),
  liquidityVaultAuthority: PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault_auth"), bankKey.toBuffer()],
    programId,
  )[0].toBase58(),
};

assertEqual("program", accountMap.programs.marginfi, expected.program);
assertEqual("group", accountMap.cloneAccounts.marginfiGroup, expected.group);
assertEqual("USDC mint", accountMap.cloneAccounts.usdcMint, expected.mint);
assertEqual("bank oracle", accountMap.cloneAccounts.bankOracle, expected.oracle);
assertEqual(
  "liquidity vault",
  accountMap.cloneAccounts.bankLiquidityVault,
  expected.liquidityVault,
);
assertEqual(
  "liquidity vault authority",
  accountMap.derivedAccounts.bankLiquidityVaultAuthority,
  expected.liquidityVaultAuthority,
);
assertEqual("CPI plan bank", cpiPlan.usdcBank, bankKey.toBase58());
assertEqual("CPI plan group", cpiPlan.group, expected.group);
assertEqual("CPI plan adapter state", cpiPlan.custody.marginfiAccountAuthority, accountMap.derivedAccounts.adapterState);
assertEqual("CPI plan adapter vault", cpiPlan.custody.adapterUnderlyingVault, accountMap.derivedAccounts.adapterVault);

const cloneAccounts = Object.values(accountMap.cloneAccounts);
if (new Set(cloneAccounts).size !== cloneAccounts.length) {
  throw new Error("fork clone account list contains duplicates");
}

const rpc = process.env.MAINNET_RPC_URL ?? "https://api.mainnet-beta.solana.com";
const connection = new Connection(rpc, "confirmed");
const [forkSlot, currentBankInfo] = await Promise.all([
  connection.getSlot("finalized"),
  connection.getAccountInfo(bankKey, "finalized"),
]);
if (!currentBankInfo || !currentBankInfo.owner.equals(programId)) {
  throw new Error("current RPC bank account is missing or has the wrong owner");
}
const currentBank = coder.accounts.decode("Bank", currentBankInfo.data);
assertEqual("current bank group", new PublicKey(currentBank.group).toBase58(), expected.group);
assertEqual("current bank mint", new PublicKey(currentBank.mint).toBase58(), expected.mint);
assertEqual(
  "current bank oracle",
  new PublicKey(currentBank.config.oracle_keys[0]).toBase58(),
  expected.oracle,
);
const referenceSo = path.join(
  repoRoot,
  "target/sbf-solana-solana/release/reference_yield_adapter.so",
);
const dispatcherSo = path.join(
  repoRoot,
  "target/sbf-solana-solana/release/yield_adapter_dispatcher.so",
);
const args = [
  "--url",
  rpc,
  "--reset",
  "--warp-slot",
  String(forkSlot),
  "--clone-upgradeable-program",
  accountMap.programs.marginfi,
  "--bpf-program",
  accountMap.programs.referenceAdapter,
  referenceSo,
  "--bpf-program",
  accountMap.programs.dispatcher,
  dispatcherSo,
  ...cloneAccounts.flatMap((account) => ["--clone", account]),
];

console.log("== MarginFi mainnet-fork account-map preflight ==");
console.log(`evidenceSlot=${accountMap.evidenceSlot}`);
console.log(`forkSlot=${forkSlot}`);
console.log(`sdk=@mrgnlabs/marginfi-client-v2@${marginfiPackage.version}`);
console.log(`bank=${bankKey.toBase58()}`);
console.log(`oracle=${expected.oracle}`);
console.log(`cloneAccounts=${cloneAccounts.length}`);
console.log("\nPowerShell validator command:");
console.log(formatPowerShellCommand(args));
console.log("\nRuntime-created accounts:");
for (const [name, description] of Object.entries(accountMap.runtimeAccounts)) {
  console.log(`- ${name}: ${description}`);
}
console.log("\nNo validator or live transaction was started.");

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

function assertEqual(label, actual, expectedValue) {
  if (actual !== expectedValue) {
    throw new Error(`${label} mismatch: actual=${actual} expected=${expectedValue}`);
  }
}

function formatPowerShellCommand(commandArgs) {
  return ["solana-test-validator", ...commandArgs]
    .map((part) => (/[\s;]/.test(part) ? `"${part.replaceAll('"', '`"')}"` : part))
    .join(" ");
}
