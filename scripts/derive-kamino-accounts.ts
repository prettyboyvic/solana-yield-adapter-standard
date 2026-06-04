/**
 * Derive the exact Main-Market USDC accounts needed for a Kamino (klend) CPI,
 * using ONLY the official @kamino-finance/klend-sdk + verified on-chain reads.
 *
 * This script does NOT implement CPI and does NOT claim a Kamino pass. It only
 * prints the account map (or explicit null/BLOCKED for anything it cannot verify
 * on-chain) so the real CPI can be wired from verified data, never guesses.
 *
 * Run on a machine with mainnet RPC access:
 *   npm install            # installs @kamino-finance/klend-sdk + @solana/web3.js
 *   KAMINO_RPC_URL=https://<mainnet-rpc> npm run kamino:derive
 *   # optional: OWNER=<adapter authority pubkey> to also derive the obligation PDA
 *
 * Sources: KaminoMarket.load / getReserve('USDC') and the Main Market address
 * 7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF are from the official klend-sdk
 * README (github.com/Kamino-Finance/klend-sdk).
 */

import * as web3 from "@solana/web3.js";
import * as klend from "@kamino-finance/klend-sdk";

const RPC = process.env.KAMINO_RPC_URL;
const MAIN_MARKET = "7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF";
const USDC_MINT = "EPjFWdd5AufqSSqeM2qzH6oEgCG1kduA3s3z2nZ7G8mm";
const KLEND_PROGRAM_ID = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";
const TOKEN_PROGRAM_ID = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID =
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

/** Stringify a PublicKey-ish value, else null. Never throws, never guesses. */
function pk(x: unknown): string | null {
  if (!x) return null;
  try {
    if (x instanceof web3.PublicKey) return x.toBase58();
    const anyx = x as { toBase58?: () => string; toString?: () => string };
    if (typeof anyx.toBase58 === "function") return anyx.toBase58();
    if (typeof anyx.toString === "function") {
      const s = anyx.toString();
      // crude base58 pubkey sanity check (32-44 chars, no 0/O/I/l)
      if (/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(s)) return s;
    }
  } catch {
    /* fall through */
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

async function main() {
  const out: Record<string, unknown> = {
    note: "Kamino Main-Market USDC account map. null/BLOCKED = not verified on-chain; do NOT hardcode guesses.",
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

  if (!RPC) {
    out.status = "BLOCKED";
    out.blocker =
      "KAMINO_RPC_URL not set. Set a mainnet RPC URL and re-run; on-chain reserve sub-accounts cannot be derived without it.";
    print(out);
    process.exit(2);
  }

  const connection = new web3.Connection(RPC, "confirmed");
  const market = await klend.KaminoMarket.load(
    connection,
    new web3.PublicKey(MAIN_MARKET),
  );
  if (!market) throw new Error("KaminoMarket.load returned null");
  if (typeof (market as { loadReserves?: () => Promise<void> }).loadReserves === "function") {
    await (market as { loadReserves: () => Promise<void> }).loadReserves();
  }

  const reserve =
    (market as { getReserve?: (s: string) => unknown }).getReserve?.("USDC") ??
    null;

  if (!reserve) {
    out.status = "BLOCKED";
    out.blocker =
      "market.getReserve('USDC') returned null on this RPC. Confirm the Main Market still lists a USDC reserve.";
    print(out);
    process.exit(2);
  }

  // Lending market authority PDA — prefer an SDK helper if present; otherwise
  // derive with the documented klend seed "lma" and flag it for confirmation.
  let lendingMarketAuthority: string | null = null;
  let authPdaMethod = "BLOCKED";
  const helper = (klend as Record<string, unknown>).lendingMarketAuthPda;
  if (typeof helper === "function") {
    try {
      const r = (helper as (...a: unknown[]) => unknown)(
        new web3.PublicKey(MAIN_MARKET),
        new web3.PublicKey(KLEND_PROGRAM_ID),
      );
      lendingMarketAuthority = pk(Array.isArray(r) ? r[0] : r);
      authPdaMethod = "klend-sdk lendingMarketAuthPda()";
    } catch {
      /* fall through to manual */
    }
  }
  if (!lendingMarketAuthority) {
    try {
      const [addr] = web3.PublicKey.findProgramAddressSync(
        [Buffer.from("lma"), new web3.PublicKey(MAIN_MARKET).toBuffer()],
        new web3.PublicKey(KLEND_PROGRAM_ID),
      );
      lendingMarketAuthority = addr.toBase58();
      authPdaMethod = 'manual seed ["lma", lendingMarket] — CONFIRM against klend IDL';
    } catch {
      authPdaMethod = "BLOCKED";
    }
  }

  out.usdcReserve =
    pk((reserve as { address?: unknown }).address) ??
    pk(get(reserve, "state.address"));
  out.reserveLiquiditySupplyVault = pk(get(reserve, "state.liquidity.supplyVault"));
  out.reserveLiquidityMint = pk(get(reserve, "state.liquidity.mintPubkey"));
  out.reserveCollateralMint = pk(get(reserve, "state.collateral.mintPubkey"));
  out.reserveDestinationDepositCollateral = pk(
    get(reserve, "state.collateral.supplyVault"),
  );
  out.reserveFeeReceiver = pk(get(reserve, "state.liquidity.feeVault"));
  out.lendingMarketAuthority = lendingMarketAuthority;
  out.lendingMarketAuthorityPdaMethod = authPdaMethod;

  // Oracles referenced by the reserve config (only those actually configured).
  out.oracles = {
    pyth: pk(get(reserve, "state.config.tokenInfo.pythConfiguration.price")),
    switchboardPrice: pk(
      get(reserve, "state.config.tokenInfo.switchboardConfiguration.priceAggregator"),
    ),
    switchboardTwap: pk(
      get(reserve, "state.config.tokenInfo.switchboardConfiguration.twapAggregator"),
    ),
    scopePriceFeed: pk(
      get(reserve, "state.config.tokenInfo.scopeConfiguration.priceFeed"),
    ),
  };

  // Obligation PDA strategy (per-user; derived, not a single static address).
  const owner = process.env.OWNER;
  let obligation: Record<string, unknown> = {
    strategy:
      "Vanilla obligation: PDA seeded by tag/id + owner + lending market + 2 placeholder mints (Pubkey::default). Use klend VanillaObligation(programId) and its toPda(market, owner). Derived per adapter authority/user.",
    derivedForOwner: null,
  };
  if (owner) {
    try {
      const VanillaObligation = (klend as Record<string, unknown>)
        .VanillaObligation as
        | (new (p: web3.PublicKey) => { toPda?: (m: web3.PublicKey, o: web3.PublicKey) => unknown })
        | undefined;
      if (VanillaObligation) {
        const vo = new VanillaObligation(new web3.PublicKey(KLEND_PROGRAM_ID));
        const pda = vo.toPda?.(
          new web3.PublicKey(MAIN_MARKET),
          new web3.PublicKey(owner),
        );
        obligation.derivedForOwner = pk(pda) ?? "BLOCKED";
      } else {
        obligation.derivedForOwner = "BLOCKED: VanillaObligation not exported by installed klend-sdk";
      }
    } catch (e) {
      obligation.derivedForOwner = `BLOCKED: ${(e as Error).message}`;
    }
  }
  out.obligation = obligation;

  // Mark any unresolved required account explicitly.
  const required = [
    "usdcReserve",
    "reserveLiquiditySupplyVault",
    "reserveCollateralMint",
    "reserveDestinationDepositCollateral",
    "lendingMarketAuthority",
  ];
  const unresolved = required.filter((k) => !out[k]);
  out.status = unresolved.length === 0 ? "DERIVED" : "PARTIAL";
  out.unresolved = unresolved;
  out.disclaimer =
    "Kamino CPI is NOT implemented and no mainnet-fork roundtrip has been run. This is account derivation only.";

  print(out);
}

function print(o: unknown) {
  console.log(JSON.stringify(o, null, 2));
}

main().catch((e) => {
  console.error(
    JSON.stringify(
      { status: "BLOCKED", blocker: (e as Error).message, cpiImplemented: false },
      null,
      2,
    ),
  );
  process.exit(1);
});
