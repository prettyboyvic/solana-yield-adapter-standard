import { describe, expect, it } from "vitest";
import {
  REFERENCE_ADAPTERS,
  adapterId,
  anchorDiscriminator,
  bytesToHex,
  encodeCurrentValue,
  encodeDeposit,
  encodeWithdraw,
  positionSeeds,
  registrySeeds,
} from "../src/index.js";

describe("adapter ABI", () => {
  it("uses Anchor-compatible 8-byte discriminators", () => {
    expect(anchorDiscriminator("deposit")).toHaveLength(8);
    expect(anchorDiscriminator("withdraw")).toHaveLength(8);
    expect(anchorDiscriminator("current_value")).toHaveLength(8);
    expect(bytesToHex(anchorDiscriminator("deposit"))).not.toEqual(
      bytesToHex(anchorDiscriminator("withdraw")),
    );
  });

  it("encodes deposit, withdraw, and current_value payloads", () => {
    const id = adapterId("kamino-usdc");
    expect(encodeDeposit(id, 1_000_000n, 999_000n)).toHaveLength(56);
    expect(encodeWithdraw(id, 500_000n, 499_000n)).toHaveLength(56);
    expect(encodeCurrentValue(id)).toHaveLength(40);
  });

  it("writes u64 values in little-endian order", () => {
    const id = adapterId("kamino-usdc");
    const data = encodeDeposit(id, 0x0102_0304_0506_0708n, 0n);
    expect(Array.from(data.subarray(40, 48))).toEqual([
      0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
    ]);
  });

  it("defines five unique reference adapters", () => {
    const unique = new Set(
      REFERENCE_ADAPTERS.map((adapter) => bytesToHex(adapter.adapterId)),
    );
    expect(REFERENCE_ADAPTERS).toHaveLength(5);
    expect(unique.size).toBe(5);
  });

  it("exposes deterministic PDA seed bytes", () => {
    const owner = new Uint8Array(32).fill(7);
    expect(registrySeeds()[0]).toEqual(new TextEncoder().encode("registry"));
    expect(positionSeeds(adapterId("kamino-usdc"), owner)).toHaveLength(3);
  });
});

