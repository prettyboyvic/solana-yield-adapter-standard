/**
 * kaminoCpiAccountPlan - assemble canonical klend account-meta plans for the
 * Kamino USDC adapter from the VERIFIED docs/kamino-derived-accounts.json.
 *
 * Source of truth:
 * - Account order and isSigner/isWritable/optional flags come from the installed
 *   @kamino-finance/klend-sdk idl.json.
 * - Optional-none placeholders and instruction discriminators are left to the
 *   official generated builders; this helper only emits account-meta plans.
 * - Account values come from docs/kamino-derived-accounts.json plus explicit
 *   caller overrides for transaction-payer-shaped accounts.
 *
 * Custody model: adapter_underlying is a state-PDA-owned USDC vault used as Kamino userSourceLiquidity.
 * Transaction shape: this helper only builds canonical account plans; final Rust may be single nested CPI or split transaction flow.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { createRequire } from "node:module";
import { PublicKey } from "@solana/web3.js";
import { farmsId } from "@hubbleprotocol/farms-sdk";

export const EXPECTED_OBLIGATION_FARM_STATE =
  "FpvYH3vrip5Zaj5C2YPF6hGC17rPC5FLNEzt1ZPBmDzy";

export const KAMINO_CPI_INSTRUCTION_NAMES = [
  "initUserMetadata",
  "initObligation",
  "refreshReserve",
  "refreshObligation",
  "refreshObligationFarmsForReserve",
  "depositReserveLiquidityAndObligationCollateralV2",
  "withdrawObligationCollateralAndRedeemReserveCollateralV2",
] as const;

export type KaminoCpiInstructionName = (typeof KAMINO_CPI_INSTRUCTION_NAMES)[number];

export type AccountMetaPlan = {
  /** klend IDL account name (camelCase). */
  name: string;
  /** Resolved base58 pubkey, or the program id when the optional slot is None. */
  pubkey: string;
  isSigner: boolean;
  isWritable: boolean;
  optional: boolean;
  /** True when an optional slot is intentionally left empty (program-id = None). */
  isNone: boolean;
};

export type InstructionPlan = {
  instruction: string;
  argNames: string[];
  accounts: AccountMetaPlan[];
};

export type KaminoCpiPlans = {
  custody: {
    obligationOwner: string;
    adapterUnderlyingVault: string;
    note: string;
  };
  txShape: string;
  plans: Record<KaminoCpiInstructionName, InstructionPlan>;
};

export class KaminoPlanError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "KaminoPlanError";
  }
}

type IdlAccount = {
  name: string;
  isMut?: boolean;
  writable?: boolean;
  isSigner?: boolean;
  signer?: boolean;
  isOptional?: boolean;
  optional?: boolean;
};
type IdlInstruction = {
  name: string;
  args?: { name: string }[];
  accounts?: IdlAccount[];
};
type Idl = { instructions: IdlInstruction[] };

const SOURCE_BACKED_KLEND_V2_LAYOUTS: Partial<Record<KaminoCpiInstructionName, IdlInstruction>> = {
  depositReserveLiquidityAndObligationCollateralV2: {
    name: "depositReserveLiquidityAndObligationCollateralV2",
    args: [{ name: "liquidityAmount" }],
    accounts: [
      { name: "owner", isSigner: true, isMut: true },
      { name: "obligation", isMut: true },
      { name: "lendingMarket" },
      { name: "lendingMarketAuthority" },
      { name: "reserve", isMut: true },
      { name: "reserveLiquidityMint" },
      { name: "reserveLiquiditySupply", isMut: true },
      { name: "reserveCollateralMint", isMut: true },
      { name: "reserveDestinationDepositCollateral", isMut: true },
      { name: "userSourceLiquidity", isMut: true },
      { name: "placeholderUserDestinationCollateral", isOptional: true },
      { name: "collateralTokenProgram" },
      { name: "liquidityTokenProgram" },
      { name: "instructionSysvarAccount" },
      { name: "obligationFarmUserState", isMut: true, isOptional: true },
      { name: "reserveFarmState", isMut: true, isOptional: true },
      { name: "farmsProgram" },
    ],
  },
  withdrawObligationCollateralAndRedeemReserveCollateralV2: {
    name: "withdrawObligationCollateralAndRedeemReserveCollateralV2",
    args: [{ name: "collateralAmount" }],
    accounts: [
      { name: "owner", isSigner: true, isMut: true },
      { name: "obligation", isMut: true },
      { name: "lendingMarket" },
      { name: "lendingMarketAuthority" },
      { name: "withdrawReserve", isMut: true },
      { name: "reserveLiquidityMint" },
      { name: "reserveSourceCollateral", isMut: true },
      { name: "reserveCollateralMint", isMut: true },
      { name: "reserveLiquiditySupply", isMut: true },
      { name: "userDestinationLiquidity", isMut: true },
      { name: "placeholderUserDestinationCollateral", isOptional: true },
      { name: "collateralTokenProgram" },
      { name: "liquidityTokenProgram" },
      { name: "instructionSysvarAccount" },
      { name: "obligationFarmUserState", isMut: true, isOptional: true },
      { name: "reserveFarmState", isMut: true, isOptional: true },
      { name: "farmsProgram" },
    ],
  },
};

/** Load the installed klend IDL (order + flags source of truth). */
export function loadKlendIdl(): Idl {
  const req = createRequire(import.meta.url);
  const main = req.resolve("@kamino-finance/klend-sdk");
  const idlPath = path.join(path.dirname(main), "idl.json");
  return JSON.parse(fs.readFileSync(idlPath, "utf8")) as Idl;
}

/** Load the installed generated-builder klend program id. */
export function loadKlendProgramId(): string {
  const req = createRequire(import.meta.url);
  const sdk = req("@kamino-finance/klend-sdk") as {
    PROGRAM_ID?: { toBase58?: () => string; toString?: () => string };
  };
  const programId = sdk.PROGRAM_ID;
  const value =
    typeof programId?.toBase58 === "function" ? programId.toBase58() : programId?.toString?.();
  return normalizeAccountValue("@kamino-finance/klend-sdk PROGRAM_ID", value);
}

/** Minimal shape of the verified derived JSON this helper consumes. */
export type DerivedKaminoAccounts = {
  cpiReady?: boolean;
  cpiPrereqStatus?: string;
  klendProgramId?: string | null;
  lendingMarket?: string | null;
  lendingMarketAuthority?: string | null;
  usdcReserve?: string | null;
  reserveLiquidityMint?: string | null;
  reserveLiquiditySupplyVault?: string | null;
  reserveCollateralMint?: string | null;
  reserveDestinationDepositCollateral?: string | null;
  tokenProgram?: string | null;
  instructionsSysvar?: string | null;
  rentSysvar?: string | null;
  systemProgram?: string | null;
  oracles?: {
    pyth?: string | null;
    switchboardPrice?: string | null;
    switchboardTwap?: string | null;
    scopePriceFeed?: string | null;
  };
  cpiPrereqs?: {
    adapterAuthority?: string | null;
    userMetadata?: string | null;
    obligation?: string | null;
    obligationArgs?: {
      tag?: number | null;
      id?: number | null;
      seed1?: string | null;
      seed2?: string | null;
    } | null;
    reserveCollateralFarmState?: string | null;
    obligationFarmState?: string | null;
  };
};

export type BuildPlanInputs = {
  derived: DerivedKaminoAccounts;
  /** State-PDA-owned USDC vault. Defaults to ATA(adapterAuthority, USDC). */
  adapterUnderlyingVault?: string;
  /** Init/farm-refresh payer. Defaults to the obligation owner (state PDA). */
  feePayer?: string;
  /** Farm-refresh crank signer. Defaults to feePayer, matching the generated action. */
  crankSigner?: string;
  /** Optional referrer metadata PDA. Omitted means klend optional-none. */
  referrerUserMetadata?: string;
};

const DEFAULT_PK = PublicKey.default.toBase58();

function normalizeAccountValue(name: string, value: unknown): string {
  if (typeof value !== "string") {
    throw new KaminoPlanError(`${name} is not a string account value: ${String(value)}`);
  }
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    throw new KaminoPlanError(`${name} is an empty account value`);
  }
  if (trimmed.startsWith("BLOCKED")) {
    throw new KaminoPlanError(`${name} is blocked: ${trimmed}`);
  }
  try {
    return new PublicKey(trimmed).toBase58();
  } catch (e) {
    throw new KaminoPlanError(`${name} is not a valid public key: ${(e as Error).message}`);
  }
}

function normalizeOptionalAccountValue(
  name: string,
  value: unknown,
  nonePubkey: string,
): { pubkey: string; isNone: boolean } {
  if (value === undefined || value === null) {
    return { pubkey: nonePubkey, isNone: true };
  }
  const pubkey = normalizeAccountValue(name, value);
  if (pubkey === DEFAULT_PK) {
    return { pubkey: nonePubkey, isNone: true };
  }
  return { pubkey, isNone: false };
}

/** Canonical ATA derivation: [owner, tokenProgram, mint] under the ATA program. */
function deriveAta(owner: string, mint: string, tokenProgram: string): string {
  const ATA_PROGRAM = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
  const [ata] = PublicKey.findProgramAddressSync(
    [
      new PublicKey(owner).toBuffer(),
      new PublicKey(tokenProgram).toBuffer(),
      new PublicKey(mint).toBuffer(),
    ],
    ATA_PROGRAM,
  );
  return ata.toBase58();
}

export function kaminoCpiAccountPlan(inputs: BuildPlanInputs): KaminoCpiPlans {
  const d = inputs.derived;
  const p = d.cpiPrereqs ?? {};

  if (d.cpiReady !== true || d.cpiPrereqStatus !== "READY") {
    throw new KaminoPlanError(
      `derived map is not CPI-ready (cpiReady=${d.cpiReady}, cpiPrereqStatus=${d.cpiPrereqStatus}); rerun kamino:derive`,
    );
  }

  const klendProgramId = loadKlendProgramId();
  if (d.klendProgramId !== undefined && d.klendProgramId !== null) {
    const derivedProgramId = normalizeAccountValue("klendProgramId", d.klendProgramId);
    if (derivedProgramId !== klendProgramId) {
      throw new KaminoPlanError(
        `derived klendProgramId ${derivedProgramId} does not match installed klend PROGRAM_ID ${klendProgramId}`,
      );
    }
  }

  const obligationFarmState = normalizeAccountValue(
    "cpiPrereqs.obligationFarmState",
    p.obligationFarmState,
  );
  if (obligationFarmState !== EXPECTED_OBLIGATION_FARM_STATE) {
    throw new KaminoPlanError(
      `unexpected obligation farm state ${obligationFarmState}; expected ${EXPECTED_OBLIGATION_FARM_STATE}`,
    );
  }

  if (p.obligationArgs?.tag !== 0 || p.obligationArgs.id !== 0) {
    throw new KaminoPlanError(
      `expected vanilla obligation args tag=0 id=0; got tag=${String(
        p.obligationArgs?.tag,
      )} id=${String(p.obligationArgs?.id)}`,
    );
  }

  const tokenProgram = normalizeAccountValue("tokenProgram", d.tokenProgram);
  const obligationOwner = normalizeAccountValue("cpiPrereqs.adapterAuthority", p.adapterAuthority);
  const feePayer = normalizeAccountValue("feePayer", inputs.feePayer ?? obligationOwner);
  const crankSigner = normalizeAccountValue("crankSigner", inputs.crankSigner ?? feePayer);
  const underlyingMint = normalizeAccountValue("reserveLiquidityMint", d.reserveLiquidityMint);
  const adapterUnderlyingVault =
    inputs.adapterUnderlyingVault ?? deriveAta(obligationOwner, underlyingMint, tokenProgram);
  const normalizedAdapterUnderlyingVault = normalizeAccountValue(
    "adapterUnderlyingVault",
    adapterUnderlyingVault,
  );

  // klend IDL account name -> resolved value. Unmapped optional slots resolve to
  // the klend program id, matching the generated klend optionalAccount helper.
  const accountValues: Record<string, unknown> = {
    owner: obligationOwner,
    obligationOwner,
    feePayer,
    crank: crankSigner,
    userMetadata: p.userMetadata,
    referrerUserMetadata: inputs.referrerUserMetadata,
    obligation: p.obligation,
    lendingMarket: d.lendingMarket,
    lendingMarketAuthority: d.lendingMarketAuthority,
    seed1Account: p.obligationArgs?.seed1,
    seed2Account: p.obligationArgs?.seed2,
    ownerUserMetadata: p.userMetadata,
    reserve: d.usdcReserve,
    pythOracle: d.oracles?.pyth,
    switchboardPriceOracle: d.oracles?.switchboardPrice,
    switchboardTwapOracle: d.oracles?.switchboardTwap,
    scopePrices: d.oracles?.scopePriceFeed,
    withdrawReserve: d.usdcReserve,
    reserveLiquidityMint: d.reserveLiquidityMint,
    reserveLiquiditySupply: d.reserveLiquiditySupplyVault,
    reserveCollateralMint: d.reserveCollateralMint,
    reserveDestinationDepositCollateral: d.reserveDestinationDepositCollateral,
    reserveSourceCollateral: d.reserveDestinationDepositCollateral,
    userSourceLiquidity: normalizedAdapterUnderlyingVault,
    userDestinationLiquidity: normalizedAdapterUnderlyingVault,
    placeholderUserDestinationCollateral: undefined,
    collateralTokenProgram: tokenProgram,
    liquidityTokenProgram: tokenProgram,
    instructionSysvarAccount: d.instructionsSysvar,
    reserveFarmState: p.reserveCollateralFarmState,
    obligationFarmUserState: obligationFarmState,
    farmsProgram: farmsId.toBase58(),
    rent: d.rentSysvar,
    systemProgram: d.systemProgram,
  };

  const idl = loadKlendIdl();
  const byName = (name: string): IdlInstruction => {
    const ix = idl.instructions.find((i) => i.name === name);
    if (!ix) {
      const sourceBacked = SOURCE_BACKED_KLEND_V2_LAYOUTS[name as KaminoCpiInstructionName];
      if (sourceBacked) return sourceBacked;
      throw new KaminoPlanError(`klend IDL is missing instruction ${name}`);
    }
    if (!ix.accounts) throw new KaminoPlanError(`klend IDL instruction ${name} has no accounts`);
    return ix;
  };

  const buildPlan = (ixName: KaminoCpiInstructionName): InstructionPlan => {
    const ix = byName(ixName);
    const accounts: AccountMetaPlan[] = ix.accounts!.map((a) => {
      const optional = Boolean(a.isOptional ?? a.optional);
      const { pubkey, isNone } = optional
        ? normalizeOptionalAccountValue(`${ixName}.${a.name}`, accountValues[a.name], klendProgramId)
        : { pubkey: normalizeAccountValue(`${ixName}.${a.name}`, accountValues[a.name]), isNone: false };
      return {
        name: a.name,
        pubkey,
        isSigner: Boolean(a.isSigner ?? a.signer),
        isWritable: Boolean(a.isMut ?? a.writable),
        optional,
        isNone,
      };
    });
    return { instruction: ixName, argNames: (ix.args ?? []).map((x) => x.name), accounts };
  };

  const plans = {} as Record<KaminoCpiInstructionName, InstructionPlan>;
  for (const ixName of KAMINO_CPI_INSTRUCTION_NAMES) {
    plans[ixName] = buildPlan(ixName);
  }

  return {
    custody: {
      obligationOwner,
      adapterUnderlyingVault: normalizedAdapterUnderlyingVault,
      note:
        "obligation owned by adapter state PDA; adapter_underlying is the state-PDA USDC vault used as klend userSourceLiquidity/userDestinationLiquidity.",
    },
    txShape:
      "NOT FINAL: this helper only builds canonical account plans; final Rust may be single nested CPI or split transaction flow.",
    plans,
  };
}

export const buildKaminoCpiPlans = kaminoCpiAccountPlan;
