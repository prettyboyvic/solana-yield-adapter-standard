# Layer C pre-flight verification (offsets, oracle, instructions)

Initial verification pass (Gate-2, option b). **No adapter/SDK/dispatcher/production changes.** Every value below is from official Drift source or decoded from the real cloned mainnet accounts in `tools/drift-litesvm-smoke/fixtures/accounts/` — nothing guessed.

**Layer C status: BLOCKED by a confirmed deployed-binary/on-chain-IDL mismatch.** Layer B passed and proved the freshly dumped mainnet binary is real Drift BPF. The Layer C dispatch preflight then proved that known-present `admin_withdraw_from_insurance_fund_vault` dispatches, while required `initialize_insurance_fund_stake` and `add_insurance_fund_stake` both return Anchor 101. The harness stops before attempting Layer C with:

```text
LAYER_C_BLOCKED: dumped Drift binary lacks IF-stake instruction handlers
```

## Sources inspected
- Drift on-chain Rust (master):
  - `programs/drift/src/state/insurance_fund_stake.rs` — `InsuranceFundStake` (`#[account(zero_copy(unsafe))] #[repr(C)]`, `SIZE = 136`).
  - `programs/drift/src/state/spot_market.rs` — `SpotMarket` (`SIZE = 776`, `MARKET_INDEX_OFFSET = 684`) and nested `InsuranceFund` (`#[zero_copy(unsafe)] #[repr(C)]`).
- Drift IDL (`sdk/src/idl/drift.json`) — instruction account orders (recorded in `audit-and-plan.md` §2).
- Real cloned mainnet accounts (finalized dump on host): `SpotMarket[0]` `6gMq3mRCKf8aP3ttTyYhuijVZ2LGi14oDsBbkgubfLB3.json`, `insurance_fund_vault[0]` `2CqkQvYxp9Mq…`, etc.

Drift's `Size::SIZE` **includes** the 8-byte Anchor discriminator; the cloned `SpotMarket[0]` is exactly 776 bytes, and its observed discriminator `64b1086ba8414127` equals the computed `sha256("account:SpotMarket")[..8]` — this validates the offset+discriminator method end-to-end. All offsets below are into the **raw account data (discriminator included at bytes 0..8)**, little-endian.

## 1. `InsuranceFundStake.if_shares` — VERIFIED
Field order (no internal padding; fields are alignment-ordered):
`disc[0:8] · authority[8:40] · if_shares[40:56] · last_withdraw_request_shares[56:72] · if_base[72:88] · last_valid_ts[88:96] · last_withdraw_request_value[96:104] · last_withdraw_request_ts[104:112] · cost_basis[112:120] · market_index[120:122] · padding[122:136]`.

- **`if_shares` = bytes `[40..56)`, `u128` LE.**
- Account discriminator (validate before decoding): `sha256("account:InsuranceFundStake")[..8]` = `6e ca 0e 2a 5f 49 5a 5f`.
- Note: `if_shares` is a private Rust field but its on-disk position is fixed. `if_base` (rebase exponent) is at `[72:88]`; if `shares_base != 0` the IF has rebased and `if_shares` must be interpreted relative to `shares_base` (currently 0 for USDC — see §2).

## 2. `SpotMarket.insurance_fund.{total_shares, unstaking_period}` — VERIFIED
`insurance_fund` begins at account-data offset **304** — confirmed two independent ways:
- Empirically: the 32 bytes at `[304:336)` equal `insurance_fund_vault[0]` (`2CqkQvYxp9Mq…`).
- By construction: `8 (disc) + 5×32 (pubkey,oracle,mint,vault,name) + 48 (HistoricalOracleData) + 40 (HistoricalIndexData) + 2×24 (PoolBalance) = 304`. (The nested sizes are confirmed correct precisely because the computed 304 matches the empirical 304.)

`InsuranceFund` layout from offset 304:
`vault[304:336] · total_shares[336:352] · user_shares[352:368] · shares_base[368:384] · unstaking_period[384:392] · last_revenue_settle_ts[392:400] · revenue_settle_period[400:408] · total_factor[408:412] · user_factor[412:416]`.

- **`insurance_fund.total_shares` = bytes `[336..352)`, `u128` LE.**
- **`insurance_fund.unstaking_period` = bytes `[384..392)`, `i64` LE.**
- Also: `user_shares` `[352:368)` u128, `shares_base` `[368:384)` u128.

Decoded from the real cloned `SpotMarket[0]`:
| field | value |
|---|---|
| total_shares | `3,841,390,135,873` |
| user_shares | `2,352,803,551,671` (< total ✓) |
| shares_base | `0` (no rebase) |
| **unstaking_period** | `1,123,200 s = exactly 13 days` |

> Layer-C/adapter consequence: the `remove_insurance_fund_stake` cooldown is **13 days**, so the clock warp must exceed it — use **≥ 14 days** for the remove step (the smoke used exactly 13).

current_value decode method (for the adapter step): `value ≈ if_vault_token_balance × if_shares ÷ total_shares` (read `if_shares` per §1, `total_shares` per §2, and the SPL token balance of `insurance_fund_vault`). Handle `shares_base` rebase if ever non-zero.

## 3. Real USDC oracle — CONFIRMED (not guessed)
`SpotMarket.oracle` = bytes `[40..72)` of the `SpotMarket` account. Decoded from the cloned `SpotMarket[0]` (self-validated: `mint[72:104]` == USDC, `vault[104:136]` == `spot_market_vault[0]`):

- **USDC oracle = `9VCioxmni2gDLv11qufWzT3RDERhQE4iY5Gf7NTfYyAV`** (read from `SpotMarket[0].oracle`).
- `oracle_source` is a later enum field in `SpotMarket`; resolve it if the IF path ever needs it (the IF add/remove instructions carry **no** oracle account, so Layer C does not require the oracle for execution — but it must be cloned if any refresh path is added).

Action: clone `9VCioxmni2gDLv11qufWzT3RDERhQE4iY5Gf7NTfYyAV` into `fixtures/accounts/` for completeness.

## 4. Instruction names, discriminators, account order — VERIFIED
Discriminators = `sha256("global:<snake_case_name>")[..8]` (method validated in §0 via the account-disc match). Account orders verbatim from the IDL (`audit-and-plan.md` §2).

### `initialize_user_stats` — disc `fe f3 48 62 fb 82 a8 d5` — args: none
`[userStats(w), state(w), authority(ro), payer(w,signer), rent, systemProgram]`

### `initialize_insurance_fund_stake` — disc `bb b3 f3 46 f8 5a 5c 93` — args: `marketIndex: u16`
`[spotMarket(ro), insuranceFundStake(w), userStats(w), state(ro), authority(ro,signer), payer(w,signer), rent, systemProgram]`

### `add_insurance_fund_stake` — disc `fb 90 73 0b de 2f 3e ec` — args: `marketIndex: u16, amount: u64`
`[state(ro), spotMarket(w), insuranceFundStake(w), userStats(w), authority(ro,signer), spotMarketVault(w), insuranceFundVault(w), driftSigner(ro), userTokenAccount(w), tokenProgram(ro)]`

(For later: `request_remove…` disc `8e 46 cc 5c 49 6a b4 34`; `remove_insurance_fund_stake` disc `80 a6 8e 09 fe bb 8f ae`.)

## Layer C implementation plan (implemented; blocked at dispatch preflight)
Extend `tools/drift-litesvm-smoke` (or a sibling bin) to run a real IF instruction sequence under litesvm:

1. Compute the three discriminators in-harness (don't hardcode) and assert they equal §4.
2. Fresh local keypair `authority`; airdrop SOL in litesvm.
3. Derive runtime PDAs: `user_stats = ["user_stats", authority]`, `if_stake = ["insurance_fund_stake", authority, 0u16le]`.
4. Stage a funded USDC token account owned by `authority` (mint USDC via `set_account`, or clone+retarget) for `add_insurance_fund_stake.userTokenAccount`.
5. Cloned program accounts already present: state, drift_signer, spot_market[0], spot_market_vault[0], insurance_fund_vault[0], USDC mint, + oracle `9VCiox…`.
6. Execute `initialize_user_stats` → `initialize_insurance_fund_stake(0)` → `add_insurance_fund_stake(0, amount)`, asserting Drift `success` logs (not `InstructionMissing`) and a non-zero `if_shares` read back from the IF-stake account at offset `[40:56]`. That is the scoped `[C] OK`.
7. Deferred to the adapter step (not Layer C): `request_remove` → clock warp **≥14 days** → `remove_insurance_fund_stake`, and switching `authority` to the adapter state PDA.

Residual risk to watch (no guess): `add_insurance_fund_stake` may require the cloned `SpotMarket[0]`/`InsuranceFund` to satisfy Drift's internal invariants at the dumped slot (e.g. paused-operation flags, revenue-settle timing). It carries no oracle account, which is favorable. If it rejects, capture the Drift error code and resolve against source before proceeding — do not fake success.

## Discriminator source — corrected/confirmed against the DEPLOYED program (Layer C run 1)

Layer C run 1 failed: all three instructions returned Anchor `InstructionFallbackNotFound` (101). Root cause was investigated against the deployed program, not just `master`.

### Method used (official, authoritative)
Installed `@drift-labs/sdk@2.155.0` (its bundled IDL `metadata.address` == `dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH`, i.e. the deployed program) and used its real Anchor `BorshInstructionCoder` (`@coral-xyz/anchor@0.28.0`) to `encode()` each instruction and read the first 8 bytes — exactly what the production SDK sends on-chain.

### Names: snake_case-derived
The IDL instruction names are **camelCase** (`initializeUserStats`), but Anchor's coder derives the discriminator from the **snake_case** form: `sha256("global:" + snake_case(name))[..8]`. (`snakeCase("initializeUserStats") = "initialize_user_stats"`.)

### Old vs corrected discriminator bytes — IDENTICAL (no change needed)
| instruction | harness bytes (run 1) | SDK coder bytes | match |
|---|---|---|---|
| `initialize_user_stats` | `fef34862fb82a8d5` | `fef34862fb82a8d5` | ✅ |
| `initialize_insurance_fund_stake` | `bbb3f346f85a5c93` | `bbb3f346f85a5c93` | ✅ |
| `add_insurance_fund_stake` | `fb90730bde2f3eec` | `fb90730bde2f3eec` | ✅ |

**The discriminators were already correct.** The discriminator source was NOT the bug.

### Initial finding: the dumped `fixtures/drift.so` lacks the IF-stake handlers
Reproduced independently with the litesvm **node** binding loading the same `fixtures/drift.so`:
- `admin_withdraw_from_insurance_fund_vault` (disc `e4d0bff6a93abdd5`) → **DISPATCHED** (reaches handler → error 102 `InstructionDidNotDeserialize`). Proves the binary is real Drift and uses standard `sha256("global:snake")` discriminators.
- `initialize_user_stats`, `initialize_insurance_fund_stake`, `add_insurance_fund_stake` → **`InstructionFallbackNotFound` (101)**.

So under the **same** standard scheme that `admin_withdraw…` matches, the IF-stake instructions are **absent** from this binary.

### Fresh mainnet re-dump + on-chain IDL verification (June 5, 2026)
1. Re-dumped the current mainnet program successfully:
   ```powershell
   solana program dump dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH fixtures\drift.so -u m
   ```
   File size remained `6,673,069` bytes; the timestamp changed from `2026-06-05 16:16:21 +07:00` to `2026-06-05 17:01:40 +07:00`. Fresh SHA-256: `72CC3062523BF87B028B5BEE163872140AD7A91F0EE32497E4159DAAA312D396`.
2. `anchor` CLI was unavailable, so the equivalent `@coral-xyz/anchor@0.28.0` `Program.fetchIdl` path fetched the program's on-chain IDL to ignored fixture `fixtures/onchain-idl.json`.
3. The on-chain IDL contains all three instructions, and its `BorshInstructionCoder` produces the same verified bytes:

   | instruction | on-chain IDL | discriminator |
   |---|---:|---|
   | `initializeUserStats` | present | `fef34862fb82a8d5` |
   | `initializeInsuranceFundStake` | present | `bbb3f346f85a5c93` |
   | `addInsuranceFundStake` | present | `fb90730bde2f3eec` |

4. Rerunning the smoke against the fresh mainnet dump still produced Anchor 101 for both required IF-stake instructions. The known-present admin control still dispatched and Layer B still passed.

Conclusion: this is not a stale local dump or discriminator bug. The currently deployed mainnet binary does not dispatch the IF-stake handlers advertised by its on-chain IDL/SDK. Layer C cannot proceed to real IF execution with this deployed binary.

The harness runs a dispatch preflight **before Layer C**. It first requires known-present `admin_withdraw_from_insurance_fund_vault` to dispatch, then probes required `initialize_insurance_fund_stake` and `add_insurance_fund_stake`. If either required IF instruction returns Anchor 101, it stops before Layer C with `LAYER_C_BLOCKED: dumped Drift binary lacks IF-stake instruction handlers`.

## Status
Offsets, oracle, account orders, and discriminators are confirmed against official source + the deployed-program SDK coder and on-chain IDL. **Layer B: PASSED. Layer C: BLOCKED by the confirmed deployed-binary/on-chain-IDL mismatch.** No Drift IF success claimed.
