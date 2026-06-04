import { describe, it, expect } from "vitest";
import {
  REFERENCE_ADAPTERS,
  forkCloneAccounts,
  PENDING,
  DERIVE,
} from "../packages/sdk/src/index.js";

// Two gates:
//  - default (no env): fork-READINESS checks run in CI/sandbox. They assert each
//    adapter's mainnet wiring is coherent enough to build a --clone command.
//  - MAINNET_FORK_LIVE=1: the on-validator deposit/current_value/withdraw roundtrip
//    is expected to be driven by scripts/run-mainnet-fork.mjs against a local
//    validator with the deployed programs. Those assertions are intentionally not
//    run inside unit tests because they require a live solana-test-validator.
const live = process.env.MAINNET_FORK_LIVE === "1";

describe("mainnet-fork readiness", () => {
  it("produces a non-empty, deduped clone-account set", () => {
    const accounts = forkCloneAccounts();
    expect(accounts.length).toBeGreaterThan(0);
    expect(new Set(accounts).size).toBe(accounts.length);
    // Never emit unresolved sentinels into a --clone command.
    for (const a of accounts) {
      expect(a).not.toBe(DERIVE);
      expect(a).not.toBe(PENDING);
    }
  });

  for (const adapter of REFERENCE_ADAPTERS) {
    const pending =
      adapter.mainnet.programId === PENDING ||
      adapter.mainnet.receiptMint === PENDING;

    it.skipIf(pending)(
      `${adapter.label}: wiring is coherent for fork CPI`,
      () => {
        const m = adapter.mainnet;
        expect(m.programId).not.toBe(PENDING);
        expect(m.underlyingMint).not.toBe(PENDING);
        // At least the program + underlying are concrete and clone-able.
        expect(m.cloneAccounts.length).toBeGreaterThan(0);
        // Anything not statically known is explicitly listed for on-machine derivation.
        if (m.receiptMint === DERIVE) {
          expect(m.derived.length).toBeGreaterThan(0);
        }
      },
    );

    it.skipIf(!pending)(`${adapter.label}: flagged PENDING on-chain verification`, () => {
      expect(pending).toBe(true);
    });
  }

  it.skipIf(!live)(
    "live roundtrip is driven by scripts/run-mainnet-fork.mjs",
    () => {
      // Placeholder marker: the real deposit/value/withdraw assertions execute in
      // the runner against a validator. If MAINNET_FORK_LIVE=1 is set inside plain
      // vitest (no validator), fail loudly rather than pretending to pass.
      throw new Error(
        "Run the live roundtrip via `node scripts/run-mainnet-fork.mjs`, not vitest.",
      );
    },
  );
});
