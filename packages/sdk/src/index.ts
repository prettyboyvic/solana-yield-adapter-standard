import { createHash } from "node:crypto";

export const DISPATCHER_PROGRAM_ID =
  "37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY";
export const REFERENCE_ADAPTER_PROGRAM_ID =
  "BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1";

export const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// Verified mainnet program ids / mints (stable, web-verified 2026-06-04).
// Sources recorded in docs/submission.md.
export const KAMINO_KLEND_PROGRAM_ID =
  "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";
export const MARGINFI_V2_PROGRAM_ID =
  "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA";
export const DRIFT_V2_PROGRAM_ID =
  "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH";
export const JLP_MINT = "27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4";

// Maple syrupUSDC on Solana is a Chainlink CCIP / token route (not a native Solana
// lending CPI). Addresses below are from official Maple docs (verified 2026-06-04).
export const MAPLE_SYRUP_USDC_MINT =
  "AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj";
export const MAPLE_CCIP_ROUTER =
  "Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C";
export const MAPLE_CCIP_POOL =
  "HrTBpF3LqSxXnjnYdR4htnBLyMHNZ6eNaDZGPundvHbm";
export const MAPLE_SYRUP_USDC_ORACLE =
  "CpNyiFt84q66665Kx64bobxZuMgZ2EecrhAJs1HikS2T";

// Accounts that are position/market-specific and must be DERIVED on-machine via
// each protocol's SDK (not a single static address). Kept explicit so the fork
// harness never silently uses a fake address.
export const DERIVE = "DERIVE_VIA_PROTOCOL_SDK_ON_MACHINE" as const;
export const PENDING = "PENDING_ONCHAIN_VERIFICATION" as const;

export const CAPABILITY_DEPOSIT = 1 << 0;
export const CAPABILITY_WITHDRAW = 1 << 1;
export const CAPABILITY_CURRENT_VALUE = 1 << 2;
export const CAPABILITY_ALL =
  CAPABILITY_DEPOSIT | CAPABILITY_WITHDRAW | CAPABILITY_CURRENT_VALUE;

export enum ProtocolKind {
  KaminoUsdc = 1,
  MarginfiUsdc = 2,
  JupiterLp = 3,
  MapleSyrup = 4,
  DriftInsuranceFund = 5,
}

export type AdapterInstructionName = "deposit" | "withdraw" | "current_value";

// SIMULATED reference instructions (virtual-yield accounting, not bounty-grade CPI).
export const SIMULATED_INSTRUCTIONS: readonly AdapterInstructionName[] = [
  "deposit",
  "withdraw",
  "current_value",
];

// Real-CPI route instruction names. These are a SEPARATE route from the simulated
// ones; the on-chain bodies currently fail loudly (not implemented), so live CPI
// must NOT be claimed as working yet.
export type AdapterCpiInstructionName =
  | "deposit_cpi"
  | "withdraw_cpi"
  | "current_value_cpi";

export const CPI_INSTRUCTIONS: readonly AdapterCpiInstructionName[] = [
  "deposit_cpi",
  "withdraw_cpi",
  "current_value_cpi",
];

export type KaminoCpiEntrypointName =
  | "kamino_init"
  | "kamino_deposit"
  | "kamino_withdraw";

export const KAMINO_CPI_ENTRYPOINTS: readonly KaminoCpiEntrypointName[] = [
  "kamino_init",
  "kamino_deposit",
  "kamino_withdraw",
];

/** Full live CPI is not implemented yet. Guards against claiming bounty completion. */
export const CPI_IMPLEMENTED = false as const;

export interface AdapterMainnetWiring {
  /** On-chain program the adapter performs CPI into. */
  programId: string;
  /** Asset the user deposits. */
  underlyingMint: string;
  /** Receipt/share token, or DERIVE/PENDING when not a single static address. */
  receiptMint: string;
  /** Concrete accounts safe to `--clone` into a local fork. */
  cloneAccounts: string[];
  /** Accounts that must be derived on-machine via the protocol SDK. */
  derived: string[];
  notes: string;
}

export interface ReferenceAdapterConfig {
  label: string;
  adapterId: Uint8Array;
  protocol: ProtocolKind;
  underlyingMint: string;
  receiptMint: string;
  capabilities: number;
  riskTier: number;
  metadataUri: string;
  mainnet: AdapterMainnetWiring;
}

export const REFERENCE_ADAPTERS: readonly ReferenceAdapterConfig[] = [
  {
    label: "kamino-usdc",
    adapterId: adapterId("kamino-usdc"),
    protocol: ProtocolKind.KaminoUsdc,
    underlyingMint: USDC_MINT,
    receiptMint: DERIVE,
    capabilities: CAPABILITY_ALL,
    riskTier: 2,
    metadataUri: "ipfs://solana-yield-adapters/kamino-usdc.json",
    mainnet: {
      programId: KAMINO_KLEND_PROGRAM_ID,
      underlyingMint: USDC_MINT,
      receiptMint: DERIVE,
      cloneAccounts: [KAMINO_KLEND_PROGRAM_ID, USDC_MINT],
      derived: ["lendingMarket", "usdcReserve", "reserveCollateralMint", "userObligation"],
      notes:
        "Derive USDC reserve + collateral (cToken) mint via @kamino-finance/klend-sdk: market.getReserve('USDC').",
    },
  },
  {
    label: "marginfi-usdc",
    adapterId: adapterId("marginfi-usdc"),
    protocol: ProtocolKind.MarginfiUsdc,
    underlyingMint: USDC_MINT,
    receiptMint: DERIVE,
    capabilities: CAPABILITY_ALL,
    riskTier: 2,
    metadataUri: "ipfs://solana-yield-adapters/marginfi-usdc.json",
    mainnet: {
      programId: MARGINFI_V2_PROGRAM_ID,
      underlyingMint: USDC_MINT,
      receiptMint: DERIVE,
      cloneAccounts: [MARGINFI_V2_PROGRAM_ID, USDC_MINT],
      derived: ["marginfiGroup", "usdcBank", "liquidityVault", "marginfiAccount"],
      notes:
        "MarginFi is bank-based (no SPL receipt mint). Derive production group + USDC bank via @mrgnlabs/marginfi-client-v2.",
    },
  },
  {
    label: "jupiter-lp",
    adapterId: adapterId("jupiter-lp"),
    protocol: ProtocolKind.JupiterLp,
    underlyingMint: USDC_MINT,
    receiptMint: JLP_MINT,
    capabilities: CAPABILITY_ALL,
    riskTier: 3,
    metadataUri: "ipfs://solana-yield-adapters/jupiter-lp.json",
    mainnet: {
      programId: "PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu",
      underlyingMint: USDC_MINT,
      receiptMint: JLP_MINT,
      cloneAccounts: [JLP_MINT, USDC_MINT],
      derived: ["jupiterPerpsPool", "custodies", "poolVaults"],
      notes:
        "Deposit USDC into the Jupiter Perps pool, receive JLP (27G8...idD4). Confirm Perps program id + pool/custody accounts on-chain before enabling CPI.",
    },
  },
  {
    label: "maple-syrup",
    adapterId: adapterId("maple-syrup"),
    protocol: ProtocolKind.MapleSyrup,
    underlyingMint: USDC_MINT,
    receiptMint: MAPLE_SYRUP_USDC_MINT,
    capabilities: CAPABILITY_ALL,
    riskTier: 3,
    metadataUri: "ipfs://solana-yield-adapters/maple-syrup.json",
    mainnet: {
      programId: MAPLE_CCIP_ROUTER,
      underlyingMint: USDC_MINT,
      receiptMint: MAPLE_SYRUP_USDC_MINT,
      cloneAccounts: [
        USDC_MINT,
        MAPLE_SYRUP_USDC_MINT,
        MAPLE_CCIP_ROUTER,
        MAPLE_CCIP_POOL,
        MAPLE_SYRUP_USDC_ORACLE,
      ],
      derived: ["ccipTokenPoolConfig", "ccipOnRampConfig", "userSyrupUsdcAta"],
      notes:
        "Maple syrupUSDC on Solana is a Chainlink CCIP / token route, NOT a native Solana lending CPI. Addresses (mint/router/pool/oracle) are verified, but the live mainnet-fork roundtrip and the actual integration are NOT complete.",
    },
  },
  {
    label: "drift-insurance-fund",
    adapterId: adapterId("drift-insurance-fund"),
    protocol: ProtocolKind.DriftInsuranceFund,
    underlyingMint: USDC_MINT,
    receiptMint: DERIVE,
    capabilities: CAPABILITY_ALL,
    riskTier: 4,
    metadataUri: "ipfs://solana-yield-adapters/drift-insurance-fund.json",
    mainnet: {
      programId: DRIFT_V2_PROGRAM_ID,
      underlyingMint: USDC_MINT,
      receiptMint: DERIVE,
      cloneAccounts: [DRIFT_V2_PROGRAM_ID, USDC_MINT],
      derived: ["driftState", "usdcSpotMarket", "insuranceFundVault", "ifStakeAccount"],
      notes:
        "IF stake is account-based (no SPL share mint). Derive state, USDC spot market (index 0), and IF vault via @drift-labs/sdk.",
    },
  },
] as const;

/** Concrete, clone-able mainnet accounts across all adapters (deduped). */
export function forkCloneAccounts(): string[] {
  const set = new Set<string>([USDC_MINT]);
  for (const a of REFERENCE_ADAPTERS) {
    for (const acc of a.mainnet.cloneAccounts) {
      if (acc !== DERIVE && acc !== PENDING) set.add(acc);
    }
  }
  return [...set];
}

export function adapterId(label: string): Uint8Array {
  return createHash("sha256").update(`solana-yield-adapter:${label}`).digest();
}

export function anchorDiscriminator(
  name:
    | AdapterInstructionName
    | AdapterCpiInstructionName
    | KaminoCpiEntrypointName,
): Uint8Array {
  return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

export function encodeDeposit(
  id: Uint8Array,
  amount: bigint | number,
  minSharesOut: bigint | number,
): Uint8Array {
  const data = new Uint8Array(8 + 32 + 8 + 8);
  data.set(anchorDiscriminator("deposit"), 0);
  writeBytes32(data, 8, id);
  writeU64LE(data, 40, amount);
  writeU64LE(data, 48, minSharesOut);
  return data;
}

export function encodeWithdraw(
  id: Uint8Array,
  shares: bigint | number,
  minAssetsOut: bigint | number,
): Uint8Array {
  const data = new Uint8Array(8 + 32 + 8 + 8);
  data.set(anchorDiscriminator("withdraw"), 0);
  writeBytes32(data, 8, id);
  writeU64LE(data, 40, shares);
  writeU64LE(data, 48, minAssetsOut);
  return data;
}

export function encodeCurrentValue(id: Uint8Array): Uint8Array {
  const data = new Uint8Array(8 + 32);
  data.set(anchorDiscriminator("current_value"), 0);
  writeBytes32(data, 8, id);
  return data;
}

export function registrySeeds(): Uint8Array[] {
  return [utf8("registry")];
}

export function adapterStateSeeds(id: Uint8Array): Uint8Array[] {
  return [utf8("adapter"), assertBytes32(id)];
}

export function positionSeeds(id: Uint8Array, ownerPublicKeyBytes: Uint8Array): Uint8Array[] {
  if (ownerPublicKeyBytes.length !== 32) {
    throw new Error("owner public key must be 32 bytes");
  }
  return [utf8("position"), assertBytes32(id), ownerPublicKeyBytes];
}

export function bytesToHex(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString("hex");
}

function utf8(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

function writeBytes32(target: Uint8Array, offset: number, value: Uint8Array): void {
  target.set(assertBytes32(value), offset);
}

function assertBytes32(value: Uint8Array): Uint8Array {
  if (value.length !== 32) {
    throw new Error(`expected 32 bytes, got ${value.length}`);
  }
  return value;
}

function writeU64LE(target: Uint8Array, offset: number, value: bigint | number): void {
  const n = typeof value === "bigint" ? value : BigInt(value);
  if (n < 0n || n > 0xffff_ffff_ffff_ffffn) {
    throw new Error("u64 out of range");
  }
  const view = new DataView(target.buffer, target.byteOffset + offset, 8);
  view.setBigUint64(0, n, true);
}

export {
  EXPECTED_OBLIGATION_FARM_STATE,
  KAMINO_CPI_INSTRUCTION_NAMES,
  KaminoPlanError,
  buildKaminoCpiPlans,
  kaminoCpiAccountPlan,
  loadKlendIdl,
  loadKlendProgramId,
} from "./kaminoCpiPlan.js";
export type {
  AccountMetaPlan,
  BuildPlanInputs,
  DerivedKaminoAccounts,
  InstructionPlan,
  KaminoCpiInstructionName,
  KaminoCpiPlans,
} from "./kaminoCpiPlan.js";
