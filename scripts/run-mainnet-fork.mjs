#!/usr/bin/env node
// Guided mainnet-fork runner. Validates preconditions and prints the exact,
// ordered commands to execute on a machine that has the Solana/Anchor toolchain.
// It deliberately does NOT fake a roundtrip: the on-validator deposit/value/
// withdraw must run against a live solana-test-validator with deployed programs.
import { execSync } from "node:child_process";

const rpc = process.env.MAINNET_RPC_URL;
if (!rpc) {
  console.error("Set MAINNET_RPC_URL (an RPC that allows account cloning).");
  process.exit(1);
}

function have(cmd) {
  try {
    execSync(process.platform === "win32" ? `where ${cmd}` : `command -v ${cmd}`, {
      stdio: "ignore",
    });
    return true;
  } catch {
    return false;
  }
}

console.log("== Mainnet-fork preflight ==");
for (const tool of ["solana", "solana-test-validator", "anchor"]) {
  console.log(`  ${tool}: ${have(tool) ? "found" : "MISSING"}`);
}

console.log("\nStep 1 — print clone command + per-protocol derivation notes:");
console.log("  npm run fork:accounts\n");

console.log("Step 2 — in terminal A, start the forked validator using the");
console.log("         printed solana-test-validator --clone ... command.\n");

console.log("Step 3 — in terminal B, build + run the readiness suite:");
console.log("  anchor build");
console.log("  npx vitest run tests/mainnet-fork.spec.ts\n");

console.log("Step 4 — for each adapter, derive the per-market accounts with that");
console.log("         protocol's SDK (klend-sdk / marginfi-client-v2 / drift sdk),");
console.log("         add them to --clone, then drive deposit -> current_value ->");
console.log("         withdraw and capture tx signatures into docs/submission.md.\n");

console.log("This runner does not assert a passing roundtrip; it sets the stage so");
console.log("the on-validator evidence you capture is real, not simulated.");
