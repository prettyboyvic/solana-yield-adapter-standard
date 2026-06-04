import { describe, expect, it } from "vitest";
import {
  REFERENCE_ADAPTERS,
  SIMULATED_INSTRUCTIONS,
  CPI_INSTRUCTIONS,
  CPI_IMPLEMENTED,
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


describe("real-CPI route is separate from simulated route", () => {
  it("does not claim live CPI is implemented", () => {
    expect(CPI_IMPLEMENTED).toBe(false);
  });

  it("defines three distinct CPI instruction names", () => {
    expect(CPI_INSTRUCTIONS).toEqual([
      "deposit_cpi",
      "withdraw_cpi",
      "current_value_cpi",
    ]);
    expect(new Set(CPI_INSTRUCTIONS).size).toBe(3);
  });

  it("gives CPI instructions different discriminators than simulated ones", () => {
    const sim = new Set(
      SIMULATED_INSTRUCTIONS.map((n) => bytesToHex(anchorDiscriminator(n))),
    );
    for (const cpi of CPI_INSTRUCTIONS) {
      // A real-CPI call can never be mistaken for / fall back to a simulated call.
      expect(sim.has(bytesToHex(anchorDiscriminator(cpi)))).toBe(false);
    }
  });

  it("keeps the simulated reference path encodable (still works for ABI tests)", () => {
    const id = adapterId("kamino-usdc");
    expect(encodeDeposit(id, 1n, 0n)).toHaveLength(56);
    expect(encodeWithdraw(id, 1n, 0n)).toHaveLength(56);
    expect(encodeCurrentValue(id)).toHaveLength(40);
  });
});
