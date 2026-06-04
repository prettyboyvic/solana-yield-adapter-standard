import { describe, it } from "vitest";
import { REFERENCE_ADAPTERS } from "../packages/sdk/src/index.js";

const runFork = Boolean(process.env.MAINNET_RPC_URL);

describe.skipIf(!runFork)("mainnet-fork reference adapters", () => {
  for (const adapter of REFERENCE_ADAPTERS) {
    it(`${adapter.label} deposits, refreshes value, and withdraws`, async () => {
      throw new Error(
        "Wire protocol-specific CPI account map before enabling this fork test.",
      );
    });
  }
});

