import { createHash } from "node:crypto";

export const DISPATCHER_PROGRAM_ID =
  "37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY";
export const REFERENCE_ADAPTER_PROGRAM_ID =
  "BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1";

export const USDC_MINT = "EPjFWdd5AufqSSqeM2qzH6oEgCG1kduA3s3z2nZ7G8mm";

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

export interface ReferenceAdapterConfig {
  label: string;
  adapterId: Uint8Array;
  protocol: ProtocolKind;
  underlyingMint: string;
  receiptMint: string;
  capabilities: number;
  riskTier: number;
  metadataUri: string;
}

export const REFERENCE_ADAPTERS: readonly ReferenceAdapterConfig[] = [
  {
    label: "kamino-usdc",
    adapterId: adapterId("kamino-usdc"),
    protocol: ProtocolKind.KaminoUsdc,
    underlyingMint: USDC_MINT,
    receiptMint: "KAMINO_RECEIPT_MINT_REPLACE_WITH_MAINNET",
    capabilities: CAPABILITY_ALL,
    riskTier: 2,
    metadataUri: "ipfs://solana-yield-adapters/kamino-usdc.json",
  },
  {
    label: "marginfi-usdc",
    adapterId: adapterId("marginfi-usdc"),
    protocol: ProtocolKind.MarginfiUsdc,
    underlyingMint: USDC_MINT,
    receiptMint: "MARGINFI_RECEIPT_MINT_REPLACE_WITH_MAINNET",
    capabilities: CAPABILITY_ALL,
    riskTier: 2,
    metadataUri: "ipfs://solana-yield-adapters/marginfi-usdc.json",
  },
  {
    label: "jupiter-lp",
    adapterId: adapterId("jupiter-lp"),
    protocol: ProtocolKind.JupiterLp,
    underlyingMint: "JUPITER_LP_UNDERLYING_REPLACE_WITH_MAINNET",
    receiptMint: "JUPITER_LP_RECEIPT_REPLACE_WITH_MAINNET",
    capabilities: CAPABILITY_ALL,
    riskTier: 3,
    metadataUri: "ipfs://solana-yield-adapters/jupiter-lp.json",
  },
  {
    label: "maple-syrup",
    adapterId: adapterId("maple-syrup"),
    protocol: ProtocolKind.MapleSyrup,
    underlyingMint: USDC_MINT,
    receiptMint: "MAPLE_SYRUP_RECEIPT_REPLACE_WITH_MAINNET",
    capabilities: CAPABILITY_ALL,
    riskTier: 3,
    metadataUri: "ipfs://solana-yield-adapters/maple-syrup.json",
  },
  {
    label: "drift-insurance-fund",
    adapterId: adapterId("drift-insurance-fund"),
    protocol: ProtocolKind.DriftInsuranceFund,
    underlyingMint: USDC_MINT,
    receiptMint: "DRIFT_IF_SHARES_REPLACE_WITH_MAINNET",
    capabilities: CAPABILITY_ALL,
    riskTier: 4,
    metadataUri: "ipfs://solana-yield-adapters/drift-insurance-fund.json",
  },
] as const;

export function adapterId(label: string): Uint8Array {
  return createHash("sha256").update(`solana-yield-adapter:${label}`).digest();
}

export function anchorDiscriminator(name: AdapterInstructionName): Uint8Array {
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
