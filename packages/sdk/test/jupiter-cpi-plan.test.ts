import { describe, expect, it } from "vitest";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { PublicKey } from "@solana/web3.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixture = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, "../fixtures/jupiter-cpi-account-plan.json"), "utf8"),
);

const PROGRAM = "PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu";
const POOL = "5BUwFW4nRbftYTDMbgxykoFWqWHPzahFSNAaaaJtVKsq";
const USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const JLP = "27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4";
const USDC_CUSTODY = "G18jKKXQwBbrHeiK3C9MRXhkHsLHf7XgCSisykV46EZa";
const USDC_CUSTODY_TOKEN = "WzWUoCmtVv7eqAbU3BfKPU3fhLP6CXR8NCJH78UK9VS";
const AUM_CUSTODIES = [
  "7xS2gz2bTp3fwCC7knJvUWTEU9Tycczu6VhJYKgi1wdz",
  "AQCGyheWPLeo6Qp9WpYS9m3Qj479t7R636N9ey1rEjEn",
  "5Pv3gM9JrFFH883SWAhvJC9RPYmo8UNxuFtv5bMMALkm",
  USDC_CUSTODY,
  "4vkNeXiYEUizLdrpdPS1eC2mccyM4NUPRtERrk6ZETkk",
];
const AUM_DOVES_AG_PRICE_ACCOUNTS = [
  "FYq2BWQ1V5P1WFBqr3qB2Kb5yHVvSv7upzKodgQE5zXh",
  "AFZnHPzy4mvVCffrVwhewHbFc93uTHvDSFrVH7GtfXF1",
  "hUqAT1KQ7eW1i6Csp9CXYtpPfSAvi835V7wKi5fRfmC",
  "6Jp2xZUTWdDD2ZyUPRzeMdc6AFQ5K3pFgZxk2EijfjnM",
  "Fgc93D641F8N2d1xLjQ4jmShuD3GE3BsCXA56KBQbF5u",
];

const EXPECTED_ORDER: Record<string, Array<[string, boolean, boolean]>> = {
  addLiquidity2: [
    ["owner", true, false],
    ["fundingAccount", false, true],
    ["lpTokenAccount", false, true],
    ["transferAuthority", false, false],
    ["perpetuals", false, false],
    ["pool", false, true],
    ["custody", false, true],
    ["custodyDovesPriceAccount", false, false],
    ["custodyPythnetPriceAccount", false, false],
    ["custodyTokenAccount", false, true],
    ["lpTokenMint", false, true],
    ["tokenProgram", false, false],
    ["eventAuthority", false, false],
    ["program", false, false],
  ],
  removeLiquidity2: [
    ["owner", true, false],
    ["receivingAccount", false, true],
    ["lpTokenAccount", false, true],
    ["transferAuthority", false, false],
    ["perpetuals", false, false],
    ["pool", false, true],
    ["custody", false, true],
    ["custodyDovesPriceAccount", false, false],
    ["custodyPythnetPriceAccount", false, false],
    ["custodyTokenAccount", false, true],
    ["lpTokenMint", false, true],
    ["tokenProgram", false, false],
    ["eventAuthority", false, false],
    ["program", false, false],
  ],
};

describe("Jupiter Perps CPI account plan", () => {
  it("pins the JLP pool and selects the USDC custody by mint", () => {
    expect(fixture.program).toBe(PROGRAM);
    expect(fixture.pool).toBe(POOL);
    expect(fixture.underlyingMint).toBe(USDC);
    expect(fixture.receiptMint).toBe(JLP);
    for (const plan of Object.values(fixture.plans) as any[]) {
      expect(plan.accounts.find((account) => account.name === "custody").pubkey).toBe(
        USDC_CUSTODY,
      );
      expect(plan.accounts.find((account) => account.name === "custodyTokenAccount").pubkey).toBe(
        USDC_CUSTODY_TOKEN,
      );
    }
  });

  it("re-derives every Jupiter PDA used by the account plan", () => {
    const program = new PublicKey(PROGRAM);
    const pool = new PublicKey(POOL);
    const usdc = new PublicKey(USDC);
    expect(
      PublicKey.findProgramAddressSync([Buffer.from("perpetuals")], program)[0].toBase58(),
    ).toBe(fixture.perpetuals);
    expect(
      PublicKey.findProgramAddressSync([Buffer.from("lp_token_mint"), pool.toBuffer()], program)[0].toBase58(),
    ).toBe(JLP);
    expect(
      PublicKey.findProgramAddressSync(
        [Buffer.from("custody"), pool.toBuffer(), usdc.toBuffer()],
        program,
      )[0].toBase58(),
    ).toBe(USDC_CUSTODY);
    expect(
      PublicKey.findProgramAddressSync(
        [Buffer.from("custody_token_account"), pool.toBuffer(), usdc.toBuffer()],
        program,
      )[0].toBase58(),
    ).toBe(USDC_CUSTODY_TOKEN);
  });

  it.each(Object.keys(EXPECTED_ORDER))(
    "matches the on-chain IDL account order and flags for %s",
    (name) => {
      const plan = fixture.plans[name];
      expect(
        plan.accounts.map((account: any) => [
          account.name,
          account.isSigner,
          account.isWritable,
        ]),
      ).toEqual(EXPECTED_ORDER[name]);
      expect(plan.discriminator).toHaveLength(8);
    },
  );

  it("has concrete clone targets and no unresolved placeholders", () => {
    expect(new Set(fixture.cloneAccounts).size).toBe(fixture.cloneAccounts.length);
    for (const key of fixture.cloneAccounts) {
      expect(() => new PublicKey(key)).not.toThrow();
    }
    const raw = JSON.stringify(fixture);
    for (const banned of ["PENDING", "REPLACE", "GENERATED_AT_RUNTIME", "TODO", "FIXME"]) {
      expect(raw.includes(banned)).toBe(false);
    }
  });

  it("appends the pool-wide AUM accounts used by live AddLiquidity2 transactions", () => {
    expect(fixture.aumRemainingAccounts.map((account: any) => account.name)).toEqual([
      ...AUM_CUSTODIES.map((_, index) => `aumCustody${index}`),
      ...AUM_DOVES_AG_PRICE_ACCOUNTS.map((_, index) => `aumDovesAgPriceAccount${index}`),
    ]);
    expect(fixture.aumRemainingAccounts.map((account: any) => account.pubkey)).toEqual([
      ...AUM_CUSTODIES,
      ...AUM_DOVES_AG_PRICE_ACCOUNTS,
    ]);
    for (const account of fixture.aumRemainingAccounts) {
      expect(account.isSigner).toBe(false);
      expect(account.isWritable).toBe(false);
      expect(fixture.cloneAccounts).toContain(account.pubkey);
    }
  });

  it("defines current value from only on-chain pool, mint, and adapter JLP balance", () => {
    expect(fixture.currentValue.formula).toBe(
      "floor(adapterJlpAmount * pool.aumUsd / jlpMintSupply)",
    );
    expect(fixture.currentValue.accounts).toEqual([
      POOL,
      JLP,
      fixture.adapterCustody.adapterJlpVault,
    ]);
  });
});
