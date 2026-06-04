import {
  REFERENCE_ADAPTERS,
  forkCloneAccounts,
  DERIVE,
  PENDING,
} from "../packages/sdk/src/index.js";

const rpc = process.env.MAINNET_RPC_URL;

if (!rpc) {
  throw new Error("Set MAINNET_RPC_URL before preparing fork account clones.");
}

const accounts = forkCloneAccounts();

console.log("Reference adapters and mainnet wiring:");
for (const adapter of REFERENCE_ADAPTERS) {
  const m = adapter.mainnet;
  console.log(`\n- ${adapter.label}`);
  console.log(`    program:   ${m.programId}`);
  console.log(`    underlying:${m.underlyingMint}`);
  console.log(`    receipt:   ${m.receiptMint}`);
  if (m.derived.length) {
    console.log(`    derive on-machine: ${m.derived.join(", ")}`);
  }
  console.log(`    note: ${m.notes}`);
}

const stillOpen = REFERENCE_ADAPTERS.filter(
  (a) => a.mainnet.programId === PENDING || a.mainnet.receiptMint === PENDING,
).map((a) => a.label);

if (stillOpen.length) {
  console.log(
    `\nPENDING on-chain verification before real CPI: ${stillOpen.join(", ")}`,
  );
}

console.log(`\nConcrete accounts to clone (${accounts.length}):`);
for (const a of accounts) console.log(`  ${a}`);

console.log(
  "\nNote: per-market reserve/bank/group/obligation/IF-stake accounts are derived\n" +
    "on-machine via each protocol SDK (shown as " +
    DERIVE +
    ") and added to --clone there.",
);

console.log("\nRun (clones the verified static accounts):");
console.log(
  [
    "solana-test-validator",
    "--url",
    rpc,
    ...accounts.flatMap((account) => ["--clone", account]),
    "--reset",
  ].join(" "),
);
