# Kamino USDC CPI — Transaction Shape & Custody Design

Status: split-transaction design approved; first-deposit init CPI implemented in
`1b490f0`. The Rust `kamino_init` entrypoint performs only the setup CPIs
(`initUserMetadata` + `initObligation`) and moves no funds. Kamino
`kamino_deposit`, `kamino_withdraw`, and real `current_value` CPI/roundtrip are
still not implemented or passing; the reference adapter's simulated instructions
remain separate and unchanged. Inputs this design builds on, all already in-repo:

- `docs/kamino-derived-accounts.json` — verified account map, `cpiPrereqStatus: READY`.
- `packages/sdk/src/kaminoCpiPlan.ts` + `packages/sdk/fixtures/kamino-cpi-account-plan.json`
  — canonical, IDL-ordered account-meta plans.
- `programs/reference_yield_adapter/tests/kamino_cpi_remaining_accounts.rs`
  — fixture-driven remaining-account validator (order + signer/writable).
- Adapter share math: `quote_deposit_shares`, `quote_withdraw_assets`,
  `total_assets` / `total_shares`, per-user `Position.shares`.

Key program facts: adapter program `BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1`;
state PDA seeds `[b"adapter", adapter_id]` (bump in `AdapterState.bump`); position
PDA `[b"position", adapter_id, user]`. For `kamino-usdc`,
`adapter_id = sha256("solana-yield-adapter:kamino-usdc")`, state PDA =
`4Y8QTjo8oBfnS3362aMkyiNawJV4ZStWrcm1PLZaoa7R` (= derived `adapterAuthority`).

---

## 1. Transaction shape decision

### Options
- **A. Single nested CPI**: one adapter instruction CPIs klend
  refresh + farm-refresh + deposit in sequence.
- **B. Split transaction**: one transaction with several *top-level* (sibling)
  instructions — non-signed refreshes built by the dispatcher/client, and the
  PDA-signed mutation(s) executed by the adapter program.

### Decision: **B (split transaction).** This is not a preference — klend's design
requires it.

- **klend introspects sibling instructions.** `deposit…` and `withdraw…` both take
  `instructionSysvarAccount`. klend uses the Instructions sysvar to verify that
  `refreshReserve` + `refreshObligation` (and the farm refresh) executed *as
  sibling instructions in the same transaction*. A single nested CPI does not
  expose the required sibling layout, so a deposit nested behind the refreshes
  inside one adapter instruction would fail klend's staleness/refresh checks.
- **Compute budget.** A USDC deposit with an ACTIVE farm needs `refreshReserve`
  (Scope) + `refreshObligation` + `refreshObligationFarmsForReserve` + the deposit
  itself. Bundled as nested CPIs from one adapter instruction this risks the
  ~1.4M CU ceiling and adds CPI depth (dispatcher→adapter→klend = depth 2; nesting
  refreshes underneath keeps depth ≤ 2 but inflates CU). Split keeps each
  instruction's CU bounded and lets us set an explicit ComputeBudget limit.
- **Account-limit / size.** Splitting spreads the (deposit 14 + refresh sets +
  farm 10) accounts across instructions instead of one oversized account vector.
- **Signer placement.** Only the obligation-owner-signed mutations must run inside
  the adapter (PDA `invoke_signed`); refreshes need no signer and the farm crank
  can be the user — so they belong at top level, built directly against klend.
- **Failure isolation / retry.** Each step fails independently with a clear klend
  error; the runner can re-simulate and rebuild without unwinding a monolith.
- **Parity with the official client.** `KaminoAction` (klend-sdk) emits refreshes
  as sibling instructions around deposit/withdraw — we mirror the proven layout.

### Canonical transaction layout (first real pass)

Deposit (first-time user adds init prelude):
```
[0] ComputeBudget.setComputeUnitLimit(N)         # N tuned from simulation
[1] ComputeBudget.setComputeUnitPrice(p)         # optional
[2] adapter.kamino_init        (CPI: initUserMetadata + initObligation)   # first deposit only
[3] klend.refreshReserve       (USDC reserve, scopePrices)                # no signer
[4] klend.refreshObligation    (lendingMarket, obligation)                # no signer
[5] klend.refreshObligationFarmsForReserve(mode=Deposit)                  # crank = user
[6] adapter.kamino_deposit     (CPI: SPL transfer user→vault + klend deposit; mint shares)
[7] klend.refreshObligationFarmsForReserve(mode=Withdraw/Sync)            # post-refresh if required
```
Withdraw mirrors this with `kamino_withdraw` at step [6] and the redeem variant.

Refreshes [3]-[5] are assembled by the dispatcher/client from the committed
fixture plan (order + flags are already canonical). Steps [2] and [6] are the only
adapter (PDA-signed) instructions.

---

## 2. Custody model

### Confirmed: state-PDA pooled vault.
The Kamino obligation is owned by the adapter **state PDA**
(`[b"adapter", adapter_id]`). This matches the existing pooled share-vault: one
on-chain Kamino position per adapter, with per-user claims tracked in
`Position.shares`. (One global obligation is justified because the adapter is
explicitly pooled — `total_assets`/`total_shares` is shared state.)

### Account ownership
| Account | Owner / authority | Notes |
|---|---|---|
| Kamino `obligation` | state PDA | `VanillaObligation(programId).toPda(market, statePda)` |
| Kamino `userMetadata` | state PDA | `userMetadataPda(statePda, klendProgram)` |
| `obligationFarmUserState` | state PDA (via obligation) | `getUserStatePDA(farmsId, reserveFarmState, obligation)` |
| `adapterUnderlyingVault` (USDC) | **state PDA** | `ATA(statePda, USDC)`; this is klend `userSourceLiquidity`/`userDestinationLiquidity` |
| collateral (cTokens) | held inside the obligation | not a separate adapter token account |
| user USDC ATA | user | only touched by the SPL transfer in/out of the vault |
| `Position` (per user) | program data PDA | accounting only; holds no tokens |

User funds never go directly to klend: on deposit the adapter SPL-transfers
`user USDC ATA → adapterUnderlyingVault`, then klend deposits *from the vault*; on
withdraw klend redeems *to the vault*, then the adapter SPL-transfers
`adapterUnderlyingVault → user USDC ATA`.

### Signer model (`invoke_signed`)
Signer seeds for every klend mutation and every vault transfer out:
```
seeds = [ b"adapter", adapter_id.as_ref(), &[state.bump] ]
```
`invoke_signed` is REQUIRED for: `initUserMetadata`, `initObligation`,
`depositReserveLiquidityAndObligationCollateral`,
`withdrawObligationCollateralAndRedeemReserveCollateral`, and the
`adapterUnderlyingVault → user` SPL transfer (vault authority = state PDA).
NOT required for: `refreshReserve`, `refreshObligation` (no signer), the
`user → adapterUnderlyingVault` transfer (user signs), and the farm crank
(`refreshObligationFarmsForReserve`, crank = user signs at top level).
No new/fake seeds are introduced — only the existing state-PDA seeds.

### Safety / share-accounting compatibility
- The state PDA is the single authority over both the Kamino position and the USDC
  vault, so value cannot leave the adapter without the program signing.
- Pooling is consistent with the existing invariant: `total_assets` ≈ USDC value of
  the obligation's collateral; `total_shares` = sum of `Position.shares`. No user
  can withdraw more than their share because redemption is quoted from pooled state.
- Per-user isolation lives in `Position`, not on Kamino — unchanged from today.

---

## 3. On-chain CPI route plan

`AdapterCpiRoute` needs the static accounts (state PDA, position, user, user USDC
ATA, `adapterUnderlyingVault`, token program, system program) plus the
Kamino-specific accounts via `remaining_accounts`, **validated against the fixture
plan** before any CPI (see §3.6).

### 3.1 First-deposit init path (`adapter.kamino_init`)
Implemented in `1b490f0` for the setup path only. It moves no funds and does not
implement deposit, withdraw, or value CPI.

Run only when `userMetadataInitialized == false` / `obligationInitialized == false`
(both currently false on mainnet). CPI, `invoke_signed` by state PDA:
1. `initUserMetadata(userLookupTable?)` — owner = state PDA, feePayer = user.
2. `initObligation(args)` — `args = obligationArgs` (tag 0, id, seed1, seed2 from the
   derived map). Owner = state PDA, feePayer = user; `seed1Account`/`seed2Account`
   per the IDL.
Idempotency: skip if the accounts already exist (check `*Initialized` flags off-chain
and/or `account.data_is_empty()` on-chain).

### 3.2 Refresh path (top-level klend, dispatcher-built)
Order is fixed and supplied as sibling instructions:
1. `refreshReserve(reserve=USDC, lendingMarket, scopePrices)` — pyth/switchboard = None.
2. `refreshObligation(lendingMarket, obligation)`.
3. `refreshObligationFarmsForReserve(mode=Deposit)` — crank = user; uses
   `reserveFarmState`, `obligationFarmUserState`, `farmsId`, rent, systemProgram.

### 3.3 Deposit path (`adapter.kamino_deposit`, CPI, PDA-signed)
1. Validate `remaining_accounts` against fixture section
   `depositReserveLiquidityAndObligationCollateral`.
2. SPL `transfer(user_usdc_ata → adapterUnderlyingVault, amount)` (user-signed).
3. CPI `depositReserveLiquidityAndObligationCollateral(liquidityAmount=amount)`,
   `invoke_signed` by state PDA; `userSourceLiquidity = adapterUnderlyingVault`.
4. Share accounting (§4): mint shares to `Position`, bump `total_assets`/`total_shares`.

### 3.4 Withdraw path (`adapter.kamino_withdraw`, CPI, PDA-signed)
1. Validate `remaining_accounts` against `withdrawObligationCollateralAndRedeemReserveCollateral`.
2. Quote assets for the requested shares; compute the collateral amount to redeem.
3. CPI `withdrawObligationCollateralAndRedeemReserveCollateral(collateralAmount)`,
   `invoke_signed`; `userDestinationLiquidity = adapterUnderlyingVault`.
4. SPL `transfer(adapterUnderlyingVault → user_usdc_ata, assets_out)` (PDA-signed).
5. Share accounting: burn shares, decrement `total_assets`/`total_shares`.

### 3.5 current_value path (read-only)
After `refreshReserve` + `refreshObligation` (siblings), read the obligation's
deposited collateral and convert to USDC via the refreshed reserve
collateral-exchange rate (klend `Reserve` exposes the cToken↔liquidity rate). Set
`total_assets` to that USD-of-collateral value; per-user value =
`quote_withdraw_assets(state, position.shares)`. No mutation CPI; emit
`AdapterValue`.

### 3.6 Remaining-account validation gate
Before any deposit/withdraw CPI the program MUST check `remaining_accounts` against
the canonical plan: count, positional pubkey order, and signer/writable flags — the
exact checks already proven in `kamino_cpi_remaining_accounts.rs`. Production note:
the on-chain validator compares against values stored in/derived from `AdapterState`
(market, reserve, obligation, vault) — it must NOT embed the fixture JSON in the BPF
binary. The fixture remains the off-chain oracle the test/dispatcher build from.
Missing / reordered / wrong-flag accounts fail loudly before any CPI.

---

## 4. Share accounting integration

Reuse the existing pooled math; replace simulated `refresh_virtual_yield` with
real Kamino value on the Kamino path (virtual yield stays only for the simulated
reference instructions, which remain clearly labelled).

- **Deposit (mint).** `shares_out = quote_deposit_shares(state, amount)` BEFORE
  incrementing totals (first deposit: `total_shares==0` ⇒ `shares_out = amount`,
  1:1). Then `position.shares += shares_out`,
  `position.principal_assets += amount`, `total_assets += amount`,
  `total_shares += shares_out`. Enforce `shares_out >= min_shares_out`.
- **current_value.** `total_assets` is set from the refreshed obligation collateral
  value (§3.5); per-user value = `quote_withdraw_assets(state, position.shares)`.
- **Withdraw (burn).** `assets_out = quote_withdraw_assets(state, shares)`;
  require `position.shares >= shares` and `assets_out >= min_assets_out`; then
  `position.shares -= shares`, `total_shares -= shares`,
  `total_assets -= assets_out`. The actual USDC returned is what klend redeemed to
  the vault — assert `redeemed >= min_assets_out` and reconcile against `assets_out`
  (use the on-chain redeemed amount as source of truth; treat any shortfall as a
  slippage error).

### Edge cases
- **First deposit:** `total_shares==0` ⇒ 1:1 shares; init path (§3.1) runs first.
- **Zero shares / zero amount:** reject `amount==0` and `shares==0` (existing
  `InvalidAmount`); `quote_withdraw_assets` already requires `total_shares>0`.
- **Rounding:** integer `mul/div` rounds down (favours the pool, never the user
  over-withdrawing); document that dust may remain. Keep `min_shares_out` /
  `min_assets_out` slippage floors.
- **Stale reserve data:** if the sibling refreshes are missing/old, klend's
  instruction-sysvar check rejects the deposit/withdraw — surface as a clear error;
  do not fall back to virtual yield on the Kamino path.
- **Partial withdraw:** supported — burn only the requested shares; collateral
  redeemed is proportional. Re-quote after each action (no caching across tx).
- **Farm desync:** if `refreshObligationFarmsForReserve` is skipped while the farm
  is ACTIVE, deposit/withdraw can fail — always include the farm refresh sibling.

---

## 5. Mainnet-fork verification plan

Environment: Windows box with Rust/Anchor + a local `solana-test-validator` forked
from mainnet (clone klend program, farms program, USDC reserve + sub-accounts,
scope, USDC mint) + the deployed adapter/dispatcher. Fund a test user with USDC.

### Roundtrip steps
1. **Build & deploy** adapter+dispatcher to the fork; register the `kamino-usdc`
   adapter; fund test user USDC ATA.
2. **First deposit:** send the deposit transaction (layout §1, incl. init prelude
   for a fresh obligation) for e.g. 10 USDC.
   - Assert: user USDC ATA −10; obligation collateral > 0; `Position.shares > 0`
     (==10e6 on the very first deposit); `total_assets`/`total_shares` updated.
3. **current_value:** send refresh + `current_value`.
   - Assert: emitted `AdapterValue.value_assets ≈ 10 USDC` (± rounding/interest);
     nonzero; monotonic vs deposit.
4. **Withdraw:** withdraw all shares.
   - Assert: user USDC ATA returns ≈ 10 USDC (≥ `min_assets_out`); `Position.shares
     == 0`; `total_shares == 0`; obligation collateral ≈ 0.

### Evidence to capture into `submission.md`
For each of the 3 transactions: the **tx signature**, the **slot**, key **balance
deltas** (user USDC, vault USDC, obligation collateral), the relevant **program
logs** (klend deposit/withdraw + adapter events `AdapterDeposit`/`AdapterValue`/
`AdapterWithdraw`), and the **fork slot** used for clones. Record the exact
ComputeBudget limit that made it pass. State plainly: "Kamino USDC real-CPI
deposit→value→withdraw roundtrip passed on mainnet-fork at slot X" — only after it
actually passes.

### Failure modes to watch
- Missing/old sibling refresh → klend staleness / instruction-sysvar error.
- Farm refresh omitted (farm ACTIVE) → deposit/withdraw failure.
- CU exhaustion → raise ComputeBudget limit; if still tight, split deposit further.
- Wrong signer (forgot `invoke_signed` or wrong bump) → "missing required signature".
- ATA not initialised (`adapterUnderlyingVault`) → create it (idempotent) in the
  init path or as a sibling `createAssociatedTokenAccountIdempotent`.
- Account order/flag drift vs fixture → caught by the §3.6 validation gate first.

---

## Completion gate
The approved design remains: transaction shape = split, custody = state-PDA
pooled vault, signer seeds = `[b"adapter", adapter_id, &[state.bump]]`. Refresh
instructions remain top-level sibling klend instructions; only PDA-signed
mutation paths belong inside the adapter.

Until `kamino_deposit`, `kamino_withdraw`, and real `current_value` are
implemented and the §5 mainnet-fork deposit -> current_value -> withdraw
roundtrip passes with evidence in `submission.md`, the honest status stays:
Kamino init CPI only; full Kamino real CPI incomplete; full bounty not claimable.
