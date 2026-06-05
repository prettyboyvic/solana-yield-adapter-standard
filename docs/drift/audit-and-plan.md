# Drift Insurance Fund — Read-Only Audit & Implementation Plan

Baseline: clean `origin/main` @ `6f46d35`. `CPI_IMPLEMENTED` stays `false`. No adapter code edited by this audit.

---

## 1. Existing adapter patterns (audit findings)

All adapters live in `programs/reference_yield_adapter/src/lib.rs`. The dispatcher ABI is in `programs/yield_adapter_dispatcher/src/lib.rs` and is **not** touched by per-protocol adapters.

### State & accounting model
- `AdapterState` (PDA `["adapter", adapter_id]`, bump stored): `protocol: u8`, `underlying_mint`, `receipt_mint`, `protocol_market`, `value_oracle`, `total_assets`, `total_shares`, `last_update_slot`, `paused`.
- `Position` (PDA `["position", adapter_id, user]`): `owner`, `adapter_id`, `shares`, `principal_assets`, `last_value_assets`, `bump`. Created via `init_if_needed` inside `AdapterCpiRoute`.
- `ProtocolKind` already reserves `DriftInsuranceFund = 5` (Rust enum + `TryFrom<u8>` + `slot_rate_bps`=30; SDK `ProtocolKind.DriftInsuranceFund = 5`).

### PDA signer pattern (uniform)
Every CPI signs with `signer_seeds = [ADAPTER_SEED, adapter_id, &[state.bump]]` via `invoke_signed`. The **state PDA is the protocol-side authority/owner** in every adapter (Kamino obligation owner; MarginFi account authority; Jupiter ATA owner).

### Vault ownership
Adapter underlying vault is an ATA: `adapter_underlying_vault_pda(state, token_program, mint)` = ATA(owner=state PDA). Guards enforce `adapter_underlying.owner == state` and exact ATA address.

### Shares accounting (real, not simulated)
- MarginFi/Jupiter withdraw measure **actual vault delta** and require `redeemed >= min_assets_out`; non-full withdraws require exact match.
- Jupiter shares = actual JLP lamports minted; `current_value = aumUsd * jlp_amount / jlp_supply`.
- Loud-fail: `resolve_cpi_route` → `MissingCpiAccounts`/`CpiNotImplemented`; `reject_unimplemented_cpi` never returns `Ok`. `current_value_cpi` dispatches by `adapter_id`, falls through to loud fail.

### Account-layout discipline
- `AccountLayoutSpec{is_signer,is_writable}` + `const fn spec(...)`; one pinned `*_LAYOUT` const per instruction. `account_layout_matches()` verifies count + per-slot flags; mismatch fails loudly.
- Instruction data: `anchor_sighash("snake_case")` = `sha256("global:<name>")[..8]` + Borsh args.
- Per-account owner + key-equality checks against pinned mainnet constants before every CPI.

### Per-adapter guards
`guard_<protocol>_cpi_route`: `adapter_id` matches the protocol constant, `state.protocol == ProtocolKind::X as u8`, `state.protocol_market == <pinned market>`, `underlying_mint == USDC_MINT`, vault ownership/mint. Withdraw guards additionally require `value_oracle != default`.

### Events
`AdapterDeposit`, `AdapterWithdraw`, `AdapterValue` on every mutation/value op, plus `*Initialized`.

---

## 2. Drift v2 IF instruction & account layouts (VERIFIED)

Source: official `drift-labs/protocol-v2` `sdk/src/idl/drift.json` (instructions) + `sdk/src/addresses/pda.ts` (seeds). Program id `dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH`.

Discriminators = `anchor_sighash` of the snake_case name (computed on-machine, never hardcoded blind).

### `initialize_user_stats` — args: none
`[userStats(w), state(w), authority(ro), payer(w,signer), rent, systemProgram]`

### `initialize_insurance_fund_stake` — args: `marketIndex: u16`
`[spotMarket(ro), insuranceFundStake(w), userStats(w), state(ro), authority(ro,signer), payer(w,signer), rent, systemProgram]`

### `add_insurance_fund_stake` — args: `marketIndex: u16, amount: u64`
`[state(ro), spotMarket(w), insuranceFundStake(w), userStats(w), authority(ro,signer), spotMarketVault(w), insuranceFundVault(w), driftSigner(ro), userTokenAccount(w), tokenProgram(ro)]`

### `request_remove_insurance_fund_stake` — args: `marketIndex: u16, amount: u64`
`[spotMarket(w), insuranceFundStake(w), userStats(w), authority(ro,signer), insuranceFundVault(w)]`

### `cancel_request_remove_insurance_fund_stake` — args: `marketIndex: u16`
`[spotMarket(w), insuranceFundStake(w), userStats(w), authority(ro,signer), insuranceFundVault(w)]`

### `remove_insurance_fund_stake` — args: `marketIndex: u16`
`[state(ro), spotMarket(w), insuranceFundStake(w), userStats(w), authority(ro,signer), insuranceFundVault(w), driftSigner(ro), userTokenAccount(w), tokenProgram(ro)]`

`authority` is the signer in all four IF-stake ops → adapter **state PDA** signs via `invoke_signed` (matches the MarginFi authority pattern). `userTokenAccount` = adapter-owned USDC vault (state PDA ATA). Note: none of `add/request_remove/remove` carry an oracle account.

---

## 3. USDC IF account map (seeds VERIFIED from pda.ts)

USDC spot market index = **0**. `mi = u16 LE`.

| Account | Derivation |
|---|---|
| Drift program | `dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH` |
| Drift state | `["drift_state"]` |
| Drift signer | `["drift_signer"]` |
| USDC spot market | `["spot_market", mi]` |
| USDC spot market vault | `["spot_market_vault", mi]` |
| USDC insurance fund vault | `["insurance_fund_vault", mi]` |
| IF stake (per authority) | `["insurance_fund_stake", authority, mi]` |
| User stats (per authority) | `["user_stats", authority]` |
| Oracle | read from SpotMarket account `oracle` field — resolve on-machine, pin into account map |
| token program / rent / system | standard sysvars |
| authority / userTokenAccount | adapter **state PDA** / state-PDA USDC ATA |

Concrete derived addresses must be frozen into `packages/sdk/fixtures/drift-mainnet-fork-account-map.json`, mirroring the MarginFi map.

---

## 4. Adapter PDA as IF-stake authority — FEASIBLE

`authority` is signer on all IF ops. Setting `authority = state PDA` + signing `[ADAPTER_SEED, adapter_id, bump]` is identical to the MarginFi `marginfi_account` authority pattern. IF stake + user_stats PDAs derive from `authority` = state PDA, so both are deterministic and adapter-owned. Confirmed compatible.

---

## 5. Implementation plan (Gate 2)

### Rust entrypoints (additive; dispatcher ABI unchanged)
1. `drift_init` — `initialize_user_stats` (if needed) + `initialize_insurance_fund_stake` (market 0), state PDA authority. Emit `DriftInitialized`.
2. `drift_deposit` — fund state-PDA USDC vault, then `add_insurance_fund_stake`. Record actual IF-share delta (`InsuranceFundStake.if_shares` pre/post) as adapter shares. `min_shares_out` floor. Emit `AdapterDeposit`.
3. `drift_request_remove` — `request_remove_insurance_fund_stake(amount)`. Emit `DriftUnstakeRequested` (request ts/shares).
4. `drift_withdraw` — `remove_insurance_fund_stake`, measure actual USDC vault delta, transfer to user, `redeemed >= min_assets_out`. Drift enforces cooldown; adapter fails loudly on Drift error. Emit `AdapterWithdraw`.
5. (optional) `drift_cancel_remove`.
6. `current_value_cpi`: add `if adapter_id == DRIFT_IF_ADAPTER_ID { return drift_current_value(...) }`.

### `current_value` decoder
`drift_current_value` = `if_vault_balance * if_shares / total_if_shares`, decoded behind discriminator checks with pinned Borsh offsets. **Offsets for `InsuranceFundStake.if_shares` and `SpotMarket.insurance_fund.{total_shares, unstaking_period}` must be resolved from Drift on-chain Rust source (`programs/drift/src/state/insurance_fund_stake.rs`, `spot_market.rs`) or the full IDL `types` block before writing — no guessing.**

### Constants to add (lib.rs)
`DRIFT_PROGRAM_ID`, `DRIFT_USDC_SPOT_MARKET_INDEX = 0`, `DRIFT_STATE`, `DRIFT_SIGNER`, `DRIFT_USDC_SPOT_MARKET`, `DRIFT_USDC_SPOT_MARKET_VAULT`, `DRIFT_USDC_IF_VAULT`, `DRIFT_USDC_ORACLE`, `DRIFT_IF_ADAPTER_ID`, plus `*_LAYOUT` consts (exact flags from §2).

### SDK / fixtures / scripts
`drift-cpi-account-plan.json`, `drift-mainnet-fork-account-map.json`, `scripts/derive-drift-accounts.ts`, `scripts/export-drift-cpi-plan.ts`.

### Runner (Rust litesvm — see gate1-feasibility.md)
`initialize_adapter(value_oracle=DRIFT_USDC_ORACLE)` → `drift_init` → `drift_deposit` → `current_value_cpi` → `drift_request_remove` → **clock warp** → `drift_withdraw` → assert `ROUNDTRIP_OK`. Only print `ROUNDTRIP_OK` if real `remove_insurance_fund_stake` succeeds.

### Tests
Rust: per-instruction layout/flag tests, discriminator tests, `drift_current_value_from_data` decoder tests, loud-fail unchanged. SDK: `drift-cpi-plan.test.ts`; `CPI_IMPLEMENTED` stays false.

### Verification target (host-side)
`cargo check --workspace` · `cargo test -p reference_yield_adapter drift` · `cargo test -p reference_yield_adapter` · `npm test` · `npm run build` · `npm run typecheck` · SBF build · Drift runner `ROUNDTRIP_OK`.

## Open items (no guessing)
1. Account struct offsets (decoder) — resolve from on-chain Drift Rust/full IDL types.
2. USDC spot-market oracle address — read from live SpotMarket[0], pin into map.
3. Clock-warp executes on Windows via Rust litesvm (node binding has no Windows prebuilt) — see gate1-feasibility.md.
