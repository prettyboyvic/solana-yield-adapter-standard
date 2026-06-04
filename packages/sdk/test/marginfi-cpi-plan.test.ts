import { describe, it, expect } from "vitest";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { PublicKey } from "@solana/web3.js";

// Offline test: validates the committed MarginFi CPI account-plan fixture against
// the IDL-proven instruction account orders / flags / discriminators. Does NOT
// import the marginfi SDK or touch RPC. Step 2 is the account plan only; no
// MarginFi Rust entrypoints exist yet and CPI_IMPLEMENTED stays false.

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturePath = path.resolve(
  __dirname,
  "../fixtures/marginfi-cpi-account-plan.json",
);
const fixture = JSON.parse(fs.readFileSync(fixturePath, "utf8"));

const RUNTIME = "GENERATED_AT_RUNTIME";
const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const EXPECTED_GROUP = "4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8";
const EXPECTED_BANK = "2s37akK2eyBbp8DZgCm7RtsaEz8eJP3Nxd4urLHQv7yB";
const MARGINFI_PROGRAM = "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA";

// IDL-proven order: [name, isSigner, isWritable]
const EXPECTED_ORDER: Record<string, [string, boolean, boolean][]> = {
  marginfi_account_initialize: [
    ["marginfi_group", false, false],
    ["marginfi_account", true, true],
    ["authority", true, false],
    ["fee_payer", true, true],
    ["system_program", false, false],
  ],
  lending_account_deposit: [
    ["group", false, false],
    ["marginfi_account", false, true],
    ["authority", true, false],
    ["bank", false, true],
    ["signer_token_account", false, true],
    ["liquidity_vault", false, true],
    ["token_program", false, false],
  ],
  lending_account_withdraw: [
    ["group", false, true],
    ["marginfi_account", false, true],
    ["authority", true, false],
    ["bank", false, true],
    ["destination_token_account", false, true],
    ["bank_liquidity_vault_authority", false, false],
    ["liquidity_vault", false, true],
    ["token_program", false, false],
  ],
};

const EXPECTED_DISCRIMINATORS: Record<string, number[]> = {
  marginfi_account_initialize: [43, 78, 61, 255, 148, 52, 249, 154],
  lending_account_deposit: [171, 94, 235, 103, 82, 64, 212, 140],
  lending_account_withdraw: [36, 72, 74, 19, 210, 210, 192, 192],
};

function isResolvedPubkey(value: string): boolean {
  try {
    // eslint-disable-next-line no-new
    new PublicKey(value);
    return true;
  } catch {
    return false;
  }
}

describe("marginfi CPI account plan fixture", () => {
  it("pins the production program, group, and USDC selection by mint", () => {
    expect(fixture.program).toBe(MARGINFI_PROGRAM);
    expect(fixture.group).toBe(EXPECTED_GROUP);
    expect(fixture.underlyingMint).toBe(USDC_MINT);
    expect(fixture.usdcBank).toBe(EXPECTED_BANK);
    expect(fixture.bankSelection.selectedBy).toBe("mint");
    expect(fixture.bankSelection.candidates).toContain(EXPECTED_BANK);
  });

  it("never silently uses an unresolved placeholder", () => {
    const raw = JSON.stringify(fixture);
    for (const banned of ["PENDING", "REPLACE", "BLOCKED", "TODO", "FIXME"]) {
      expect(raw.includes(banned)).toBe(false);
    }
  });

  it("resolves every account pubkey or marks it generatedAtRuntime", () => {
    for (const [, plan] of Object.entries(fixture.plans) as [string, any][]) {
      const slots = [...plan.accounts, ...(plan.healthRemainingAccounts ?? [])];
      for (const acc of slots) {
        expect(typeof acc.pubkey).toBe("string");
        expect(acc.pubkey.length).toBeGreaterThan(0);
        if (acc.generatedAtRuntime === true) {
          expect(acc.pubkey).toBe(RUNTIME);
        } else {
          expect(acc.pubkey).not.toBe(RUNTIME);
          expect(isResolvedPubkey(acc.pubkey)).toBe(true);
        }
      }
    }
  });

  it.each(Object.keys(EXPECTED_ORDER))(
    "matches the IDL account order, flags, and discriminator for %s",
    (ix) => {
      const plan = fixture.plans[ix];
      expect(plan).toBeTruthy();
      expect(plan.discriminator).toEqual(EXPECTED_DISCRIMINATORS[ix]);
      const got = plan.accounts.map(
        (a: any) => [a.name, a.isSigner, a.isWritable] as [string, boolean, boolean],
      );
      expect(got).toEqual(EXPECTED_ORDER[ix]);
    },
  );

  it("exposes the adapter state PDA as the deposit/withdraw authority", () => {
    for (const ix of ["lending_account_deposit", "lending_account_withdraw"]) {
      const authority = fixture.plans[ix].accounts.find(
        (a: any) => a.name === "authority",
      );
      expect(authority.isSigner).toBe(true);
      expect(authority.pubkey).toBe(fixture.custody.marginfiAccountAuthority);
    }
  });

  it("includes the withdraw health remaining accounts [bank, oracle]", () => {
    const health = fixture.plans.lending_account_withdraw.healthRemainingAccounts;
    expect(health.map((a: any) => a.name)).toEqual(["bank", "oracle"]);
    expect(health[0].pubkey).toBe(EXPECTED_BANK);
    expect(health[0].isWritable).toBe(false);
    expect(health[1].generatedAtRuntime).toBe(true);
    expect(health[1].pubkey).toBe(RUNTIME);
  });
});
