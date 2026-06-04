import { describe, it, expect } from "vitest";
import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { PublicKey, type TransactionInstruction } from "@solana/web3.js";
import {
  EXPECTED_OBLIGATION_FARM_STATE,
  KAMINO_CPI_INSTRUCTION_NAMES,
  kaminoCpiAccountPlan,
  loadKlendIdl,
  loadKlendProgramId,
  type DerivedKaminoAccounts,
  type InstructionPlan,
  type KaminoCpiInstructionName,
} from "../src/kaminoCpiPlan.js";

const require = createRequire(import.meta.url);
const klendInstructions = require("@kamino-finance/klend-sdk/dist/idl_codegen/instructions");
const klendTypes = require("@kamino-finance/klend-sdk/dist/idl_codegen/types");
const BN = require("bn.js") as typeof import("bn.js");

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const derivedPath = path.resolve(__dirname, "../../../docs/kamino-derived-accounts.json");
const fixturePath = path.resolve(__dirname, "../fixtures/kamino-cpi-account-plan.json");

// Runs offline by default: consumes the committed, verified derived JSON and the
// installed klend idl.json/generated builders. No mainnet RPC or validator.
const derived = JSON.parse(fs.readFileSync(derivedPath, "utf8")) as DerivedKaminoAccounts;

type GeneratedAccounts = Record<string, PublicKey>;

function cloneDerived(): DerivedKaminoAccounts {
  return JSON.parse(JSON.stringify(derived)) as DerivedKaminoAccounts;
}

function generatedAccounts(plan: InstructionPlan): GeneratedAccounts {
  return Object.fromEntries(
    plan.accounts.map((account) => [account.name, new PublicKey(account.pubkey)]),
  );
}

function generatedInstruction(
  name: KaminoCpiInstructionName,
  plan: InstructionPlan,
): TransactionInstruction {
  const accounts = generatedAccounts(plan);
  switch (name) {
    case "initUserMetadata":
      return klendInstructions.initUserMetadata(
        { userLookupTable: PublicKey.default },
        accounts,
      ) as TransactionInstruction;
    case "initObligation":
      return klendInstructions.initObligation(
        {
          args: {
            tag: derived.cpiPrereqs?.obligationArgs?.tag,
            id: derived.cpiPrereqs?.obligationArgs?.id,
          },
        },
        accounts,
      ) as TransactionInstruction;
    case "refreshReserve":
      return klendInstructions.refreshReserve(accounts) as TransactionInstruction;
    case "refreshObligation":
      return klendInstructions.refreshObligation(accounts) as TransactionInstruction;
    case "refreshObligationFarmsForReserve":
      return klendInstructions.refreshObligationFarmsForReserve(
        { mode: klendTypes.ReserveFarmKind.Collateral.discriminator },
        accounts,
      ) as TransactionInstruction;
    case "depositReserveLiquidityAndObligationCollateralV2":
      return manualKlendInstruction(name, plan, new BN(1));
    case "withdrawObligationCollateralAndRedeemReserveCollateralV2":
      return manualKlendInstruction(name, plan, new BN(1));
  }
}

function manualKlendInstruction(
  name: KaminoCpiInstructionName,
  plan: InstructionPlan,
  amount: InstanceType<typeof BN>,
): TransactionInstruction {
  const data = Buffer.alloc(16);
  anchorSighash(name).copy(data, 0);
  data.writeBigUInt64LE(BigInt(amount.toString()), 8);
  return {
    programId: new PublicKey(loadKlendProgramId()),
    keys: plan.accounts.map((account) => ({
      pubkey: new PublicKey(account.pubkey),
      isSigner: account.isSigner,
      isWritable: account.isWritable,
    })),
    data,
  } as TransactionInstruction;
}

function anchorSighash(name: string): Buffer {
  const snake = name.replace(/[A-Z]/g, (m) => `_${m.toLowerCase()}`);
  return createHash("sha256").update(`global:${snake}`).digest().subarray(0, 8);
}

describe("kamino derived account map", () => {
  it("is the verified READY input", () => {
    expect(derived.cpiReady).toBe(true);
    expect(derived.cpiPrereqStatus).toBe("READY");
    expect(derived.cpiPrereqs?.obligationFarmState).toBe(EXPECTED_OBLIGATION_FARM_STATE);
  });
});

describe("kaminoCpiAccountPlan", () => {
  const accountPlan = kaminoCpiAccountPlan({ derived });
  const idl = loadKlendIdl();

  it("keeps the committed Rust-bridge fixture in sync with the helper output", () => {
    const fixture = JSON.parse(fs.readFileSync(fixturePath, "utf8"));
    expect(fixture).toEqual({
      schema: "kamino-cpi-account-plan/v1",
      source: "docs/kamino-derived-accounts.json",
      generatedBy: "packages/sdk/src/kaminoCpiPlan.ts:kaminoCpiAccountPlan",
      rustBridge:
        "Use accountPlan.plans.*.accounts as canonical Kamino remaining-account plans after AdapterCpiRoute fixed accounts; this fixture does not imply live CPI is implemented.",
      accountPlan,
    });
  });

  it("builds every expected klend CPI account plan", () => {
    expect(Object.keys(accountPlan.plans)).toEqual([...KAMINO_CPI_INSTRUCTION_NAMES]);
  });

  it("uses a state-PDA-owned adapter_underlying vault for Kamino liquidity accounts", () => {
    expect(accountPlan.custody.obligationOwner).toBe(derived.cpiPrereqs?.adapterAuthority);

    const deposit = accountPlan.plans.depositReserveLiquidityAndObligationCollateralV2.accounts;
    const withdraw =
      accountPlan.plans.withdrawObligationCollateralAndRedeemReserveCollateralV2.accounts;
    expect(deposit.find((a) => a.name === "userSourceLiquidity")?.pubkey).toBe(
      accountPlan.custody.adapterUnderlyingVault,
    );
    expect(withdraw.find((a) => a.name === "userDestinationLiquidity")?.pubkey).toBe(
      accountPlan.custody.adapterUnderlyingVault,
    );
  });

  it("never emits null, undefined, empty, or BLOCKED account values", () => {
    for (const plan of Object.values(accountPlan.plans)) {
      for (const account of plan.accounts) {
        expect(account.pubkey, `${plan.instruction}.${account.name}`).toEqual(
          expect.any(String),
        );
        expect(account.pubkey.trim(), `${plan.instruction}.${account.name}`).not.toBe("");
        expect(account.pubkey, `${plan.instruction}.${account.name}`).not.toMatch(/^BLOCKED/);
        expect(() => new PublicKey(account.pubkey)).not.toThrow();
      }
    }
  });

  it("rejects non-ready or unresolved derived inputs", () => {
    const notReady = cloneDerived();
    notReady.cpiReady = false;
    expect(() => kaminoCpiAccountPlan({ derived: notReady })).toThrow(/not CPI-ready/);

    const wrongFarm = cloneDerived();
    wrongFarm.cpiPrereqs!.obligationFarmState = PublicKey.default.toBase58();
    expect(() => kaminoCpiAccountPlan({ derived: wrongFarm })).toThrow(
      /unexpected obligation farm state/,
    );

    const blocked = cloneDerived();
    blocked.cpiPrereqs!.obligation = "BLOCKED: missing";
    expect(() => kaminoCpiAccountPlan({ derived: blocked })).toThrow(/blocked/);

    const empty = cloneDerived();
    empty.cpiPrereqs!.userMetadata = "";
    expect(() => kaminoCpiAccountPlan({ derived: empty })).toThrow(/empty account value/);
  });

  it("matches the official klend IDL account names, arg names, and flags", () => {
    for (const name of KAMINO_CPI_INSTRUCTION_NAMES) {
      const plan = accountPlan.plans[name];
      const ix = idl.instructions.find((candidate) => candidate.name === name);
      if (!ix) {
        expect(name.endsWith("V2"), `only source-backed v2 layouts may be absent from SDK 3.2.x IDL`).toBe(
          true,
        );
        continue;
      }
      expect(plan.argNames).toEqual((ix!.args ?? []).map((arg) => arg.name));
      expect(plan.accounts.map((account) => account.name)).toEqual(
        ix!.accounts!.map((account) => account.name),
      );

      plan.accounts.forEach((account, i) => {
        const idlAccount = ix!.accounts![i];
        expect(account.isSigner, `${name}.${account.name} signer`).toBe(
          Boolean(idlAccount.isSigner ?? idlAccount.signer),
        );
        expect(account.isWritable, `${name}.${account.name} writable`).toBe(
          Boolean(idlAccount.isMut ?? idlAccount.writable),
        );
        expect(account.optional, `${name}.${account.name} optional`).toBe(
          Boolean(idlAccount.isOptional ?? idlAccount.optional),
        );
      });
    }
  });

  it("matches generated/source-backed klend builders for account order, flags, program id, and discriminators", () => {
    for (const name of KAMINO_CPI_INSTRUCTION_NAMES) {
      const plan = accountPlan.plans[name];
      const ix = generatedInstruction(name, plan);

      expect(ix.programId.toBase58(), name).toBe(loadKlendProgramId());
      expect(ix.data.subarray(0, 8), `${name} discriminator`).toHaveLength(8);
      expect(
        ix.keys.map((key) => ({
          pubkey: key.pubkey.toBase58(),
          isSigner: key.isSigner,
          isWritable: key.isWritable,
        })),
      ).toEqual(
        plan.accounts.map((account) => ({
          pubkey: account.pubkey,
          isSigner: account.isSigner,
          isWritable: account.isWritable,
        })),
      );
    }
  });

  it("uses official klend optional-none placeholders", () => {
    const klendProgramId = loadKlendProgramId();
    const refreshReserve = accountPlan.plans.refreshReserve.accounts;
    expect(refreshReserve.find((a) => a.name === "pythOracle")?.pubkey).toBe(klendProgramId);
    expect(refreshReserve.find((a) => a.name === "switchboardPriceOracle")?.pubkey).toBe(
      klendProgramId,
    );
    expect(refreshReserve.find((a) => a.name === "switchboardTwapOracle")?.pubkey).toBe(
      klendProgramId,
    );

    const deposit = accountPlan.plans.depositReserveLiquidityAndObligationCollateralV2.accounts;
    expect(deposit.find((a) => a.name === "placeholderUserDestinationCollateral")?.pubkey).toBe(
      klendProgramId,
    );
  });

  it("treats null optional oracle accounts as klend optional-none placeholders", () => {
    const withNullOracles = cloneDerived();
    withNullOracles.oracles = {
      ...withNullOracles.oracles,
      pyth: null,
      switchboardPrice: null,
      switchboardTwap: null,
      scopePriceFeed: null,
    };

    const nullOraclePlan = kaminoCpiAccountPlan({ derived: withNullOracles });
    const klendProgramId = loadKlendProgramId();
    const refreshReserve = nullOraclePlan.plans.refreshReserve.accounts;

    expect(refreshReserve.find((a) => a.name === "pythOracle")?.pubkey).toBe(klendProgramId);
    expect(refreshReserve.find((a) => a.name === "switchboardPriceOracle")?.pubkey).toBe(
      klendProgramId,
    );
    expect(refreshReserve.find((a) => a.name === "switchboardTwapOracle")?.pubkey).toBe(
      klendProgramId,
    );
  });
});
