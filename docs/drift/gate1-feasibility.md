# Gate 1 — Drift clock-warpable harness feasibility

**Verdict: FEASIBLE — the runner must be a Rust `litesvm` harness, NOT a node `.mjs`.** No adapter code edited. Kamino/MarginFi/Jupiter untouched.

## What was proven (executed against the real litesvm engine)
Reproducible scripts in this folder: `litesvm-clockwarp-proof.mjs`, `litesvm-accountclone-proof.mjs` (run with `npm i litesvm` on a Linux/macOS host).

- **Clock warp works.** `getClock()` → `setClock()` advanced `unixTimestamp` by exactly **1,123,200s = 13 days** and read back (`>=13d? true`). This is the core requirement to pass `SpotMarket.insurance_fund.unstaking_period`.
- **Account cloning works.** `setAccount({address,lamports,data,programAddress,executable})` injected a full account blob and `getAccount` read it back intact (data + owner preserved). This is how cloned mainnet SpotMarket / IF vault / state accounts are loaded.
- **Real program execution path present.** `addProgramFromFile`/`addProgram` (load dumped Drift `.so`), `sendTransaction` (execute real instructions), `warpToSlot`.

## Rust API verified (from `LiteSVM/litesvm` crate source — not guessed)
`crates/litesvm/src/lib.rs`: `set_account`, `set_sysvar::<T>` (Clock), `get_sysvar::<T>`, `add_program_from_file`, `add_program`, `send_transaction`, `warp_to_slot`.

## Decisive Windows finding
The **node** `litesvm` binding ships prebuilt napi binaries for **darwin + linux only** (`x86_64/aarch64 -apple-darwin`, `-unknown-linux-gnu/musl`). **No `win32-x64-msvc` build** and no Windows `optionalDependencies` entry. A node `.mjs` Drift runner would fail to load on the Windows host.

The **Rust `litesvm` crate** is pure Rust, compiles natively on Windows, and the user already has the Rust + `cargo build-sbf` toolchain. → Drift roundtrip harness should be a **Rust integration test / bin** (e.g. `programs/reference_yield_adapter/tests/drift_mainnet_fork.rs`, gated behind a feature or `#[ignore]` so normal `cargo test` is unaffected). The other adapters' node runners stay as-is.

## Favorable risk note (oracle staleness)
Drift's `add_/request_remove_/remove_insurance_fund_stake` account lists contain **no oracle account**, so warping the clock +13 days should not trip oracle-staleness checks on the IF lifecycle. De-risks the remove path.

## Residual items confirmable ONLY on the Windows host
This audit sandbox has no cargo/rustc, no solana CLI, and no RPC/general network (proxy blocks all but npm + the fetch tool). Therefore NOT done here:
1. Executing the **real Drift BPF program** against cloned accounts (needs dumped `.so` + same-slot snapshots).
2. `cargo check/test`, the **SBF build**, the **Drift runner ROUNDTRIP_OK**.

Host-side steps to validate end-to-end:
- `solana program dump dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH drift.so`; clone the IF account set at one finalized slot (state, drift_signer, spot_market[0], spot_market_vault, if_vault, USDC mint, oracle).
- Align litesvm `with_feature_set` to mainnet so cloned program feature gates match.
- Confirm `remove_insurance_fund_stake` succeeds post-warp.

## Consequence for the plan
Option A stands with one correction: **runner = Rust litesvm**, not node. All Gate-2 code can be written here, but the verification evidence (cargo/SBF/runner) must be produced on the Windows host.
