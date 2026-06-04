/**
 * Derive the exact Main-Market USDC accounts needed for a Kamino (klend) CPI,
 * using ONLY the official @kamino-finance/klend-sdk + verified on-chain reads.
 *
 * Selection is by UNDERLYING MINT (USDC), not by a guessed symbol string. If the
 * USDC reserve cannot be matched, the script returns BLOCKED together with a full
 * enumeration of every reserve the market actually contains, so nothing is guessed.
 *
 * This does NOT implement CPI and does NOT claim a Kamino pass.
 *
 * Run on a machine with mainnet RPC access:
 *   npm install
 *   KAMINO_RPC_URL=https://<mainnet-rpc> npm run kamino:derive
 *   # adapterAuthority is derived from the adapter state PDA (pooled vault owner).
 *   # Diagnostic override only: set ADAPTER_AUTHORITY=<pubkey> AND
 *   # ALLOW_ADAPTER_AUTHORITY_OVERRIDE=1 (marked NOT_FINAL); without the flag it fails.
 *   # optional: KAMINO_OUT=docs/kamino-derived-accounts.json to also write the JSON
 *
 * Sources: KaminoMarket.load / getReserves / getReserveByMint / getLendingMarketAuthority
 * and Main Market 7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF are from the installed
 * @kamino-finance/klend-sdk (github.com/Kamino-Finance/klend-sdk).
 */

import * as fs from "node:fs";
import * as web3 from "@solana/web3.js";
import * as klend from "@kamino-finance/klend-sdk";
import {
  REFERENCE_ADAPTER_PROGRAM_ID,
  adapterId,
  adapterStateSeeds,
} from "../packages/sdk/src/index.js";
import { farmsId, getUserStatePDA } from "@hubbleprotocol/farms-sdk";

const RPC = process.env.KAMINO_RPC_URL;
const MAIN_MARKET = "7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF";
const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const KLEND_PROGRAM_ID = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";
const TOKEN_PROGRAM_ID = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID =
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
// Mainnet slot duration (~450ms). Required positional arg of KaminoMarket.load.
const RECENT_SLOT_DURATION_MS = 450;

/** Stringify a PublicKey-ish value, else null. Never throws, never guesses. */
function pk(x: unknown): string | null {
  if (x === undefined || x === null) return null;
  try {
    if (x instanceof web3.PublicKey) return x.toBase58();
    const anyx = x as { toBase58?: () => string; toString?: () => string };
    if (typeof anyx.toBase58 === "function") return anyx.toBase58();
    if (typeof anyx.toString === "function") {
      const s = anyx.toString();
      if (/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(s)) return s;
    }
  } catch {
    /* ignore */
  }
  return null;
}

function get(obj: unknown, path: string): unknown {
  return path.split(".").reduce<unknown>((acc, key) => {
    if (acc && typeof acc === "object" && key in (acc as object)) {
      return (acc as Record<string, unknown>)[key];
    }
    return undefined;
  }, obj);
}

/** Call a 0-arg method by name if it exists, swallow errors. */
function call(obj: unknown, method: string): unknown {
  const fn = (obj as Record<string, unknown> | null)?.[method];
  if (typeof fn === "function") {
    try {
      return (fn as () => unknown).call(obj);
    } catch {
      /* ignore */
    }
  }
  return undefined;
}

function reserveDebug(r: unknown) {
  return {
    address: pk((r as { address?: unknown }).address) ?? pk(call(r, "getAddress")),
    symbol: (r as { symbol?: string }).symbol ?? null,
    liquidityMint: pk(call(r, "getLiquidityMint")) ?? pk(get(r, "state.liquidity.mintPubkey")),
    collateralMint: pk(call(r, "getCTokenMint")) ?? pk(get(r, "state.collateral.mintPubkey")),
    liquiditySupplyVault: pk(get(r, "state.liquidity.supplyVault")),
    collateralSupplyVault: pk(get(r, "state.collateral.supplyVault")),
    feeVault: pk(get(r, "state.liquidity.feeVault")),
    oracles: {
      pyth: pk(get(r, "state.config.tokenInfo.pythConfiguration.price")),
      switchboardPrice: pk(get(r, "state.config.tokenInfo.switchboardConfiguration.priceAggregator")),
      scopePriceFeed: pk(get(r, "state.config.tokenInfo.scopeConfiguration.priceFeed")),
    },
  };
}

// Canonical persisted account map. The FINAL emit writes here by default so the
// exact object printed to stdout is what lands on disk, without relying on the
// caller remembering to set KAMINO_OUT. KAMINO_OUT still overrides the path.
const DEFAULT_OUT = "docs/kamino-derived-accounts.json";

function emit(o: unknown, code = 0, persist = false): never {
  const json = JSON.stringify(o, null, 2);
  console.log(json);
  // Write the SAME object that was just printed. Early BLOCKED exits only write
  // when KAMINO_OUT is explicitly set (persist=false) so they never clobber the
  // canonical file; the final result emits with persist=true.
  const outPath = process.env.KAMINO_OUT ?? (persist ? DEFAULT_OUT : undefined);
  if (outPath) {
    try {
      fs.writeFileSync(outPath, json + "\n");
      console.error(`(wrote ${outPath})`);
    } catch (e) {
      console.error(`(failed to write ${outPath}: ${(e as Error).message})`);
    }
  }
  process.exit(code);
}

const base: Record<string, unknown> = {
  note: "Kamino Main-Market USDC account map, selected by underlying mint. null/BLOCKED = not verified; no guesses.",
  source: "official @kamino-finance/klend-sdk + on-chain read",
  cpiImplemented: false,
  klendProgramId: KLEND_PROGRAM_ID,
  lendingMarket: MAIN_MARKET,
  underlyingMint: USDC_MINT,
  tokenProgram: TOKEN_PROGRAM_ID,
  associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
  systemProgram: web3.SystemProgram.programId.toBase58(),
  instructionsSysvar: web3.SYSVAR_INSTRUCTIONS_PUBKEY.toBase58(),
  rentSysvar: web3.SYSVAR_RENT_PUBKEY.toBase58(),
};

async function main() {
  if (!RPC) {
    emit(
      {
        ...base,
        status: "BLOCKED",
        blocker: "KAMINO_RPC_URL not set. Set a mainnet RPC URL and re-run.",
      },
      2,
    );
  }

  const connection = new web3.Connection(RPC as string, "confirmed");
  const market = await klend.KaminoMarket.load(
    connection,
    new web3.PublicKey(MAIN_MARKET),
    RECENT_SLOT_DURATION_MS,
    new web3.PublicKey(KLEND_PROGRAM_ID),
    false,
    true,
  );
  if (!market) {
    emit({ ...base, status: "BLOCKED", blocker: "KaminoMarket.load returned null" }, 2);
  }

  // Ensure reserves are loaded, then enumerate via the real SDK method.
  if (typeof (market as { loadReserves?: () => Promise<void> }).loadReserves === "function") {
    try {
      await (market as { loadReserves: () => Promise<void> }).loadReserves();
    } catch {
      /* reserves may already be loaded by load(withReserves=true) */
    }
  }

  const allReserves =
    (call(market, "getReserves") as unknown[] | undefined) ??
    Array.from(
      ((market as { reserves?: Map<unknown, unknown> }).reserves?.values?.() ?? []) as Iterable<unknown>,
    );

  const debugList = allReserves.map(reserveDebug);

  // Select USDC strictly by mint (method first, then mint-equality scan).
  let reserve: unknown =
    (market as { getReserveByMint?: (m: web3.PublicKey) => unknown }).getReserveByMint?.(
      new web3.PublicKey(USDC_MINT),
    ) ?? undefined;
  if (!reserve) {
    reserve = allReserves.find((r) => {
      const mint = pk(call(r, "getLiquidityMint")) ?? pk(get(r, "state.liquidity.mintPubkey"));
      return mint === USDC_MINT;
    });
  }

  if (!reserve) {
    emit(
      {
        ...base,
        status: "BLOCKED",
        blocker:
          "No reserve in the Main Market matches the USDC mint. See reserveEnumeration for what the market actually contains.",
        reserveCount: allReserves.length,
        reserveEnumeration: debugList,
      },
      2,
    );
  }

  // Verification guard: the selected reserve's liquidity mint MUST equal the
  // canonical USDC mint. Fail loudly rather than ever emit a DERIVED map for the
  // wrong asset (guards against a mismatched/corrupted mint constant).
  const selectedMint =
    pk(call(reserve, "getLiquidityMint")) ?? pk(get(reserve, "state.liquidity.mintPubkey"));
  if (selectedMint !== USDC_MINT) {
    emit(
      {
        ...base,
        status: "BLOCKED",
        blocker: `Selected reserve liquidity mint ${selectedMint} != canonical USDC ${USDC_MINT}. Refusing to emit a derived map for the wrong asset.`,
        selectedReserve: pk((reserve as { address?: unknown }).address) ?? pk(call(reserve, "getAddress")),
        reserveCount: allReserves.length,
      },
      2,
    );
  }

  // Lending-market authority: prefer the SDK accessor, then the seeds helper.
  let lendingMarketAuthority = pk(call(market, "getLendingMarketAuthority")) ?? null;
  let authMethod = lendingMarketAuthority ? "market.getLendingMarketAuthority()" : "BLOCKED";
  if (!lendingMarketAuthority) {
    const helper = (klend as Record<string, unknown>).lendingMarketAuthPda;
    if (typeof helper === "function") {
      try {
        const r = (helper as (...a: unknown[]) => unknown)(
          new web3.PublicKey(MAIN_MARKET),
          new web3.PublicKey(KLEND_PROGRAM_ID),
        );
        lendingMarketAuthority = pk(Array.isArray(r) ? r[0] : r);
        authMethod = "klend-sdk lendingMarketAuthPda()";
      } catch {
        /* ignore */
      }
    }
  }

  const map: Record<string, unknown> = {
    ...base,
    usdcReserve: pk((reserve as { address?: unknown }).address) ?? pk(call(reserve, "getAddress")),
    reserveLiquidityMint: pk(call(reserve, "getLiquidityMint")) ?? pk(get(reserve, "state.liquidity.mintPubkey")),
    reserveLiquiditySupplyVault: pk(get(reserve, "state.liquidity.supplyVault")),
    reserveFeeReceiver: pk(get(reserve, "state.liquidity.feeVault")),
    reserveCollateralMint: pk(call(reserve, "getCTokenMint")) ?? pk(get(reserve, "state.collateral.mintPubkey")),
    reserveDestinationDepositCollateral: pk(get(reserve, "state.collateral.supplyVault")),
    lendingMarketAuthority,
    lendingMarketAuthorityMethod: authMethod,
    oracles: {
      pyth: pk(get(reserve, "state.config.tokenInfo.pythConfiguration.price")),
      switchboardPrice: pk(get(reserve, "state.config.tokenInfo.switchboardConfiguration.priceAggregator")),
      switchboardTwap: pk(get(reserve, "state.config.tokenInfo.switchboardConfiguration.twapAggregator")),
      scopePriceFeed: pk(get(reserve, "state.config.tokenInfo.scopeConfiguration.priceFeed")),
    },
  };

  // ---- CPI prerequisites: obligation / userMetadata / farm state ----
  // The obligation + userMetadata are owned by the adapter *state* PDA (the pooled
  // share-vault authority the reference program can invoke_signed for). Derived from
  // the SAME seeds Rust uses: [b"adapter", adapter_id], adapter_id =
  // sha256("solana-yield-adapter:kamino-usdc").
  const isAddr = (v: unknown): v is string =>
    typeof v === "string" && /^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(v);

  const KAMINO_ADAPTER_LABEL = "kamino-usdc";
  const adapterIdBytes = adapterId(KAMINO_ADAPTER_LABEL);
  const adapterProgramId = new web3.PublicKey(REFERENCE_ADAPTER_PROGRAM_ID);
  const stateSeeds = adapterStateSeeds(adapterIdBytes).map((b) => Buffer.from(b));
  const [statePda] = web3.PublicKey.findProgramAddressSync(stateSeeds, adapterProgramId);
  const derivedAuthority = statePda.toBase58();

  // Env override is a DIAGNOSTIC-ONLY path; it must opt in explicitly and is never
  // treated as final. Using it without the opt-in flag fails loudly.
  const overrideEnv = process.env.ADAPTER_AUTHORITY ?? null;
  const allowOverride = process.env.ALLOW_ADAPTER_AUTHORITY_OVERRIDE === "1";
  if (overrideEnv && !allowOverride) {
    emit(
      {
        ...base,
        status: "BLOCKED",
        blocker:
          "ADAPTER_AUTHORITY override set without ALLOW_ADAPTER_AUTHORITY_OVERRIDE=1. Refusing to use a non-derived authority.",
        derivedAuthority,
      },
      2,
    );
  }
  const adapterAuthority = overrideEnv ?? derivedAuthority;
  const adapterAuthoritySource: "derived(state-pda)" | "env-override(NOT_FINAL)" =
    overrideEnv ? "env-override(NOT_FINAL)" : "derived(state-pda)";

  const programIdPk = new web3.PublicKey(KLEND_PROGRAM_ID);
  const marketPk = new web3.PublicKey(MAIN_MARKET);
  const DEFAULT_PK = web3.PublicKey.default.toBase58();

  // Farm status read straight from the on-chain reserve account (no guessing).
  const farmCollateral = pk(get(reserve, "state.farmCollateral"));
  let farmStatus: "NONE" | "ACTIVE" | "UNKNOWN" | "BLOCKED" = "UNKNOWN";
  let reserveCollateralFarmState: string | null = null;
  if (farmCollateral === DEFAULT_PK) {
    farmStatus = "NONE";
  } else if (isAddr(farmCollateral)) {
    farmStatus = "ACTIVE";
    reserveCollateralFarmState = farmCollateral;
  } else {
    farmStatus = "UNKNOWN";
  }

  const cpi: Record<string, unknown> = {
    adapterAuthority,
    adapterAuthoritySource,
    adapterProgramId: REFERENCE_ADAPTER_PROGRAM_ID,
    adapterIdHex: Buffer.from(adapterIdBytes).toString("hex"),
    derivedStatePda: derivedAuthority,
    userMetadata: "BLOCKED",
    userMetadataInitialized: null,
    obligation: "BLOCKED",
    obligationInitialized: null,
    obligationArgs: null,
    obligationStrategy:
      "Vanilla obligation (tag 0) owned by adapterAuthority via klend VanillaObligation(programId).toPda(market, adapterAuthority).",
    farmStatus,
    reserveCollateralFarmState,
    // obligationFarmUserState is derived below via the official farms-sdk helper
    // when the reserve has an ACTIVE collateral farm; null when farm is NONE.
    obligationFarmState: null,
    obligationFarmStateInitialized: null,
  };

  {
    try {
      const authPk = new web3.PublicKey(adapterAuthority);

      // userMetadata PDA (official seeds helper) + on-chain init check.
      const umHelper = (klend as Record<string, unknown>).userMetadataPda;
      if (typeof umHelper === "function") {
        const r = (umHelper as (...a: unknown[]) => unknown)(authPk, programIdPk);
        cpi.userMetadata = pk(Array.isArray(r) ? r[0] : r) ?? "BLOCKED";
      } else {
        cpi.userMetadata = "BLOCKED: userMetadataPda not exported by installed klend-sdk";
      }
      try {
        const um = await (
          market as { getUserMetadata?: (u: web3.PublicKey) => Promise<[unknown, unknown]> }
        ).getUserMetadata?.(authPk);
        cpi.userMetadataInitialized = um ? um[1] !== null : null;
      } catch {
        /* leave null */
      }

      // Vanilla obligation PDA (official class) + init args + on-chain init check.
      const VanillaObligation = (klend as Record<string, unknown>).VanillaObligation as
        | (new (p: web3.PublicKey) => {
            toPda: (m: web3.PublicKey, u: web3.PublicKey) => unknown;
            toArgs: () => { tag: number; id: number; seed1: unknown; seed2: unknown };
          })
        | undefined;
      if (VanillaObligation) {
        const vo = new VanillaObligation(programIdPk);
        const oblPda = vo.toPda(marketPk, authPk);
        cpi.obligation = pk(oblPda) ?? "BLOCKED";
        try {
          const a = vo.toArgs();
          cpi.obligationArgs = { tag: a.tag, id: a.id, seed1: pk(a.seed1), seed2: pk(a.seed2) };
        } catch {
          /* ignore */
        }
        try {
          const obl = await (
            market as { getObligationByAddress?: (pp: web3.PublicKey) => Promise<unknown> }
          ).getObligationByAddress?.(oblPda as web3.PublicKey);
          cpi.obligationInitialized = obl !== undefined ? obl !== null : null;
        } catch {
          /* leave null */
        }

        // obligationFarmUserState PDA via the OFFICIAL @hubbleprotocol/farms-sdk
        // helper getUserStatePDA(farmsId, farmState, owner) — seeds [b"user",
        // farmState, owner]; owner = the obligation. Same derivation klend-sdk
        // uses internally. Created by the first-deposit flow (like the obligation).
        if (farmStatus === "ACTIVE" && isAddr(reserveCollateralFarmState)) {
          try {
            const farmUserState = getUserStatePDA(
              farmsId,
              new web3.PublicKey(reserveCollateralFarmState),
              oblPda as web3.PublicKey,
            );
            cpi.obligationFarmState = pk(farmUserState) ?? "BLOCKED";
            try {
              const info = await connection.getAccountInfo(farmUserState);
              cpi.obligationFarmStateInitialized = info !== null;
            } catch {
              /* leave null */
            }
          } catch (e) {
            cpi.obligationFarmState = `BLOCKED: farm user-state derivation failed: ${(e as Error).message}`;
          }
        }
      } else {
        cpi.obligation = "BLOCKED: VanillaObligation not exported by installed klend-sdk";
      }
    } catch (e) {
      cpi.blocker = `CPI prereq derivation failed: ${(e as Error).message}`;
    }
  }
  map.cpiPrereqs = cpi;

  // CPI-ready when obligation + userMetadata PDAs resolve and the farm side is
  // satisfied: farm NONE, or farm ACTIVE with the obligationFarmUserState PDA
  // derived. These PDAs are created by the first-deposit flow, so a derived
  // (not-yet-initialized) PDA still counts as ready.
  const farmReady =
    farmStatus === "NONE" || (farmStatus === "ACTIVE" && isAddr(cpi.obligationFarmState));
  const cpiReady =
    isAddr(cpi.obligation) && isAddr(cpi.userMetadata) && farmReady;
  map.cpiPrereqStatus = cpiReady ? "READY" : "BLOCKED";
  map.cpiReady = cpiReady;

  const required = [
    "usdcReserve",
    "reserveLiquiditySupplyVault",
    "reserveCollateralMint",
    "reserveDestinationDepositCollateral",
    "lendingMarketAuthority",
  ];
  const unresolved = required.filter((k) => !map[k]);
  map.status = unresolved.length === 0 ? "DERIVED" : "PARTIAL";
  map.unresolved = unresolved;
  map.reserveCount = allReserves.length;
  map.reserveEnumeration = debugList;
  map.disclaimer =
    "Account derivation only. CPI_IMPLEMENTED remains false; scoped Kamino live-fork evidence is recorded separately in docs/submission.md.";

  // Exit codes: 3 = base account map incomplete; 4 = base ok but CPI prereqs not
  // ready (loud signal); 0 = base ok and CPI prereqs READY.
  const baseOk = unresolved.length === 0;
  emit(map, !baseOk ? 3 : map.cpiReady ? 0 : 4, true);
}

main().catch((e) => {
  emit({ ...base, status: "BLOCKED", blocker: (e as Error).message }, 1);
});
