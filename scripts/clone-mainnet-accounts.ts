import { REFERENCE_ADAPTERS } from "../packages/sdk/src/index.js";

const rpc = process.env.MAINNET_RPC_URL;

if (!rpc) {
  throw new Error("Set MAINNET_RPC_URL before preparing fork account clones.");
}

const commonAccounts = [
  "EPjFWdd5AufqSSqeM2qzH6oEgCG1kduA3s3z2nZ7G8mm",
];

const protocolAccounts = [
  "KAMINO_RESERVE_AND_MARKET_ACCOUNTS_REPLACE",
  "MARGINFI_BANK_AND_GROUP_ACCOUNTS_REPLACE",
  "JUPITER_POOL_AND_VAULT_ACCOUNTS_REPLACE",
  "MAPLE_POOL_AND_RECEIPT_ACCOUNTS_REPLACE",
  "DRIFT_STATE_SPOT_MARKET_AND_INSURANCE_FUND_ACCOUNTS_REPLACE",
];

const accounts = [...commonAccounts, ...protocolAccounts].filter(
  (account) => !account.endsWith("_REPLACE"),
);

console.log("Reference adapters:");
for (const adapter of REFERENCE_ADAPTERS) {
  console.log(`- ${adapter.label}`);
}

if (accounts.length === 0) {
  console.log("No concrete protocol accounts configured yet.");
  process.exit(0);
}

console.log("\nRun:");
console.log(
  [
    "solana-test-validator",
    "--url",
    rpc,
    ...accounts.flatMap((account) => ["--clone", account]),
    "--reset",
  ].join(" "),
);

