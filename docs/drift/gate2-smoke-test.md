# Gate 2 (step 1) — Drift litesvm smoke test (host-run)

Minimal harness proving the Rust `litesvm` path can support the Drift Insurance Fund roundtrip. **This is a smoke test, not the adapter.** It must be RUN ON THE WINDOWS HOST to produce evidence — it cannot be executed in the audit sandbox (no cargo/RPC there). Do not treat Drift integration as proven until Layer B prints `[B] OK` / `SMOKE_OK` on host.

Crate: `tools/drift-litesvm-smoke/` — self-contained (declares its own `[workspace]`), so it does not affect `cargo check --workspace`, `cargo build-sbf`, or the two on-chain program crates.

## Status: Layer A PASSED on host (Windows)
Evidence (host `cargo run --release`, vendored OpenSSL via Strawberry Perl; no NASM
needed):
```
Finished `release` profile [optimized] target(s) in 10m 07s
[A1] real System-Program transfer executed; dest balance = 1000000 lamports
[A2] Clock warped: unix_timestamp 0 -> 1123200 (delta 13 days)
[A] OK
SMOKE_OK (Layer A only): native execution + clock warp proven. Set DRIFT_SO to also run Layer B against the real Drift program.
```
This proves on Windows: litesvm executes transactions natively and `Clock.unix_timestamp`
can be advanced 13 days (> the IF unstaking period). **Layer B (real Drift BPF) is
NOT yet run** — no Drift integration is claimed until Layer B prints
`Program dRiftyHA… invoke` and `[B] OK`.

## What it proves
- **Layer A** (no artifacts): executes a real System-program transfer under litesvm and warps `Clock.unix_timestamp` ≥ 13 days. Proves native execution + clock warp on Windows.
- **Layer B** (with the real Drift `.so`): loads the dumped Drift BPF, injects cloned mainnet accounts, warps the clock, and executes a transaction against the Drift program — asserting via **program logs** (`Program dRiftyHA… invoke`) that the BPF actually ran. An in-program rejection still proves execution; a loader error fails the test loudly.

## Prerequisites (host)
- Rust stable toolchain (`rustup`, normal host target — NOT platform-tools).
- Solana CLI (`solana`) with mainnet RPC access (for the dump/clone in Layer B).
- **Perl + a C compiler on PATH** (for the vendored OpenSSL build — see below). On
  Windows: Strawberry Perl + the MSVC "Build Tools for Visual Studio".

## OpenSSL build note (Windows) — root cause CONFIRMED
`openssl` is **not** pulled via TLS/RPC. Reverse-tracing the resolved `Cargo.lock`
(475 packages) shows the only consumer is **`solana-secp256r1-program`** (the
secp256r1 precompile), reached as:

```
drift-litesvm-smoke -> litesvm 0.6.1 -> solana-precompiles
                    -> solana-secp256r1-program 2.2.3 -> openssl 0.10 -> openssl-sys
```

In `solana-secp256r1-program` (verified in 2.2.3 and 3.0.0) `openssl` is a
**non-optional** dependency for the host target — the only feature is
`openssl-vendored`, there is no way to disable it. litesvm 0.6.1 depends on
`solana-precompiles` (and `solana-transaction` with the `precompiles` feature)
**non-optionally**, with **no feature to drop precompiles**. `native-tls` has zero
dependents; `reqwest` (pulled only by `solana-metrics`) uses rustls. Therefore:

> **OpenSSL is structural to litesvm's runtime and cannot be removed by any
> Cargo-only edit in this crate.** The same is true of `solana-program-test` and
> Mollusk (all pull the precompiles).

Building on Windows therefore requires OpenSSL to be available one of two ways,
both of which need a prerequisite beyond Rust + MSVC:
- **Vendored** (current `Cargo.toml`: `openssl = { features = ["vendored"] }`):
  builds OpenSSL from source — needs **Perl** (e.g. Strawberry Perl) + a C
  compiler on PATH. No system OpenSSL lib needed.
- **System/prebuilt**: install an OpenSSL dev lib (vcpkg or Shining Light) and set
  `OPENSSL_DIR` — no Perl needed.

The only way to use **no** Windows prerequisites is to run the harness under
**WSL2 / Linux**, where OpenSSL builds out of the box (system lib + perl present),
or to use the prebuilt `litesvm-linux-x64` node binding inside WSL2.

### Decision required (do not proceed until chosen)
- **A — install Strawberry Perl** on Windows (small, portable, no admin); keep the
  vendored line; `cargo run --release` then builds vendored OpenSSL. Stays fully on
  Windows.
- **B — run the harness under WSL2** (Ubuntu): `cargo run --release` builds with the
  system OpenSSL; no changes to the Windows toolchain.

Layer A cannot pass on Windows with only Rust + MSVC because the litesvm runtime
itself requires OpenSSL.

## Chosen path: A — Strawberry Perl (vendored OpenSSL)
Keep the vendored `openssl` line in `tools/drift-litesvm-smoke/Cargo.toml`. Install
the minimum host prerequisite (Perl), then rebuild.

1. Install Strawberry Perl (portable, no admin needed). Either:
   ```powershell
   winget install StrawberryPerl.StrawberryPerl
   ```
   or download from https://strawberryperl.com and run the MSI.
2. Open a NEW PowerShell window (so PATH refreshes) and verify:
   ```powershell
   perl --version
   ```
3. Rebuild from clean:
   ```powershell
   cd D:\claudeCode\repo-hunter\solana-yield-adapter-standard\tools\drift-litesvm-smoke
   cargo clean
   cargo run --release
   ```
4. If the build now fails with `nasm` not found (openssl-src uses NASM for asm on
   MSVC), install it too and reopen the terminal:
   ```powershell
   winget install NASM.NASM
   # if `nasm --version` still fails, add its dir to PATH for the session:
   $env:PATH += ";C:\Program Files\NASM"
   ```
   then re-run `cargo run --release`.

Expected minimum (Layer A):
```
[A1] real System-Program transfer executed; dest balance = 1000000 lamports
[A2] Clock warped: ...
[A] OK
SMOKE_OK (Layer A only)
```
Proceed to Layer B (real Drift `.so`) only after the crate builds cleanly and
Layer A prints `[A] OK`.

## Exact Windows commands (PowerShell)

### Layer A only (fastest sanity — no network needed)
```powershell
cd D:\claudeCode\repo-hunter\solana-yield-adapter-standard\tools\drift-litesvm-smoke
cargo run --release
```
Expected output (values for balances/timestamps are deterministic except keys):
```
Drift litesvm smoke test (Gate 2). This does NOT prove full Drift integration.

== Layer A: native litesvm execution + clock warp ==
[A1] real System-program transfer executed; dest balance = 1000000 lamports
[A2] Clock warped: unix_timestamp <t0> -> <t0+1123200> (delta 13 days)
[A] OK

SMOKE_OK (Layer A only): native execution + clock warp proven. Set DRIFT_SO to also run Layer B against the real Drift program.
```

### Layer B — real Drift BPF
1. Dump the Drift program and clone the USDC IF account set at one finalized slot:
```powershell
cd D:\claudeCode\repo-hunter\solana-yield-adapter-standard\tools\drift-litesvm-smoke
mkdir fixtures, fixtures\accounts -Force

# Dump the on-chain Drift program (.so)
solana program dump dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH fixtures\drift.so --url mainnet-beta

# Clone the IF-related accounts (USDC spot market index 0). Derive the exact
# addresses with scripts/derive-drift-accounts.ts in the full Gate-2 step; for the
# smoke test, the program-execution proof does not require a coherent set.
# Example (Drift state PDA shown; repeat --output json -o for each address):
solana account 5zpq7DvB6UdFFvpmBPspGPNfUGoBRRCE2HHg5u3gxcsN --output json -o fixtures\accounts\state.json --url mainnet-beta
# (state, drift_signer, spot_market[0], spot_market_vault, insurance_fund_vault,
#  USDC mint, oracle — add each as a separate file in fixtures\accounts\)
```
2. Run with Layer B enabled:
```powershell
$env:DRIFT_SO = "fixtures\drift.so"
$env:DRIFT_ACCOUNTS_DIR = "fixtures\accounts"
$env:WARP_DAYS = "13"
cargo run --release
```
Expected (key line is the Drift program-invoke log + `[B] OK`):
```
== Layer B: real Drift BPF program ==
[B1] loaded Drift program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH from fixtures\drift.so
[B2] injected N cloned account(s) from fixtures\accounts
[B3] clock unix_timestamp at execution = <warped>
[B4] Drift tx failed as expected: <InstructionError ...>
---- program logs ----
    Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH invoke [1]
    Program log: ...
    Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH failed: ...
----------------------
[B] OK — real Drift BPF executed under litesvm after a 13-day clock warp

SMOKE_OK: real Drift BPF execution + clock warp proven on host.
```

## Failure modes (the test fails loudly, never fakes success)
- `SMOKE_FAIL: add_program_from_file failed` → bad/missing `.so`.
- `SMOKE_FAIL: Drift BPF did NOT execute (no program-invoke log)` → loader/feature-set mismatch; litesvm may need `with_feature_set` aligned to mainnet, or a `.so` built for a different loader. **Not** proven — report back and we adjust before Gate 2 full.
- Cargo version-resolution error on `litesvm`/`solana-sdk` → nudge versions in `tools/drift-litesvm-smoke/Cargo.toml` (try `litesvm = "0.6.1"`, `solana-sdk = "2.2"`); these are the only deps and isolated from the workspace.

## Layer B preparation (real Drift program) — exact flow

Scope: this is **smoke-test evidence**, proving the real Drift BPF loads and
executes under litesvm after a clock warp. It is NOT the IF adapter and NOT a
roundtrip; success = a `Program dRiftyHA… invoke` log + `[B] OK`.

### 1. Dump the real Drift v2 program `.so`
```powershell
cd D:\claudeCode\repo-hunter\solana-yield-adapter-standard\tools\drift-litesvm-smoke
mkdir fixtures, fixtures\accounts -Force
solana program dump dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH fixtures\drift.so --url mainnet-beta
```
Drift v2 is an upgradeable BPF program; `solana program dump` writes the executable
bytes of the current program data account to `fixtures\drift.so`. Verify it is
non-trivial (hundreds of KB to a few MB):
```powershell
(Get-Item fixtures\drift.so).Length
```

### 2. `DRIFT_SO` path flow
The harness enables Layer B when `DRIFT_SO` is set, calls
`add_program_from_file(dRiftyHA…, $DRIFT_SO)`, then injects every `*.json` in
`DRIFT_ACCOUNTS_DIR` via `set_account`, warps the clock, and sends one transaction
to the Drift program id. So:
```powershell
$env:DRIFT_SO = "fixtures\drift.so"
$env:DRIFT_ACCOUNTS_DIR = "fixtures\accounts"
$env:WARP_DAYS = "13"
```

### 3. Real Drift USDC IF accounts to clone (market index 0)
Deterministic PDAs (derived from the verified seeds in `docs/drift/audit-and-plan.md`
§3; program `dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH`):

| Role | Address | Seeds |
|---|---|---|
| Drift state | `5zpq7DvB6UdFFvpmBPspGPNfUGoBRRCE2HHg5u3gxcsN` | `["drift_state"]` |
| Drift signer | `JCNCMFXo5M5qwUPg2Utu1u6YWp3MbygxqBsBeXXJfrw` | `["drift_signer"]` |
| USDC spot market | `6gMq3mRCKf8aP3ttTyYhuijVZ2LGi14oDsBbkgubfLB3` | `["spot_market", 0u16le]` |
| USDC spot market vault | `GXWqPpjQpdz7KZw9p7f5PX2eGxHAhvpNXiviFkAB8zXg` | `["spot_market_vault", 0u16le]` |
| USDC insurance fund vault | `2CqkQvYxp9Mq4PqLvAQ1eryYxebUh4Liyn5YMDtXsYci` | `["insurance_fund_vault", 0u16le]` |
| USDC mint | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | (fixed) |
| USDC oracle | read from SpotMarket[0] `oracle` field | not a PDA — see below |

The oracle is not derivable; read it from the cloned SpotMarket account (or via
`@drift-labs/sdk`) and clone that account too. Clone each at the SAME finalized slot:
```powershell
$accts = @(
  "5zpq7DvB6UdFFvpmBPspGPNfUGoBRRCE2HHg5u3gxcsN",   # state
  "JCNCMFXo5M5qwUPg2Utu1u6YWp3MbygxqBsBeXXJfrw",    # drift_signer
  "6gMq3mRCKf8aP3ttTyYhuijVZ2LGi14oDsBbkgubfLB3",   # spot_market[0]
  "GXWqPpjQpdz7KZw9p7f5PX2eGxHAhvpNXiviFkAB8zXg",   # spot_market_vault[0]
  "2CqkQvYxp9Mq4PqLvAQ1eryYxebUh4Liyn5YMDtXsYci",   # insurance_fund_vault[0]
  "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"    # USDC mint
)
foreach ($a in $accts) { solana account $a --output json -o "fixtures\accounts\$a.json" --url mainnet-beta }
# then read SpotMarket[0].oracle and clone it as well into fixtures\accounts\
```
Note on the FIRST Layer B invocation: the bare program-execution smoke (empty-data
instruction) needs only `fixtures\drift.so` to be loaded — cloned accounts are not
strictly required to prove the BPF executes. The account set above is what the
subsequent real IF instructions (`add_/request_remove_/remove_insurance_fund_stake`)
will consume, so cloning them now de-risks the next step. The IF-stake and
user-stats PDAs derive from the adapter state PDA authority and are created at
runtime, not cloned.

### 4. Run Layer B
```powershell
cargo run --release
```
Treat as proven ONLY if output contains `Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH invoke [1]`
and ends with `[B] OK`. If `[B4]` shows no invoke log, it is a loader/feature-set
mismatch (align `with_feature_set`/`with_mainnet_features` per the comment in
`main.rs`) — report back, do not treat as success.

## Status: Layer B PASSED on host (real Drift BPF executed)
Evidence (host `cargo run --release` with `DRIFT_SO` + 6 cloned accounts, 13-day warp):
```
== Layer B: real Drift BPF program ==
[B1] loaded Drift program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH from fixtures\drift.so
[B2] injected 6 cloned account(s) from fixtures\accounts
[B3] clock unix_timestamp at execution = 1123200
[B4] Drift tx failed as expected: InstructionError(0, Custom(100))
---- program logs ----
    Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH invoke [1]
    Program log: AnchorError occurred. Error Code: InstructionMissing. Error Number: 100. Error Message: 8 byte instruction identifier not provided.
    Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH consumed 1760 of 200000 compute units
    Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH failed: custom program error: 0x64
----------------------
[B] OK — real Drift BPF executed under litesvm after a 13-day clock warp
SMOKE_OK: real Drift BPF execution + clock warp proven on host.
```
Interpretation: `Program … invoke [1]` + the Anchor `InstructionMissing` (error 100 /
0x64) prove the **real Drift program executed inside litesvm** after the clock warp
and reached its Anchor instruction dispatcher (it rejected the empty 8-byte
discriminator, as expected). The cloned mainnet accounts loaded without error and the
warped clock was active at execution. No feature-set tuning was needed.

**Scope:** this is Gate-2 SMOKE evidence only — it proves the harness primitives
(load real BPF + clone accounts + warp clock + execute). It is **NOT** Drift IF
integration: no real IF instruction has been invoked, no stake created, no value
read, no roundtrip. Drift IF success is claimed only by Layer C below.

## Local-only fixtures (never committed)
`.gitignore` excludes all heavy/runtime smoke artifacts:
`tools/drift-litesvm-smoke/fixtures/` (incl. `fixtures/drift.so` and
`fixtures/accounts/*.json`) and `tools/drift-litesvm-smoke/target/`. Do not commit
the dumped `.so` or cloned account JSONs.

## Layer C plan — first REAL Drift IF instruction smoke (implemented; blocked at dispatch preflight)

Goal: invoke one real Drift IF instruction end-to-end under litesvm (still a scoped
smoke, still NOT the adapter), printing Drift program logs and a scoped `[C] OK`.

Sequence to build (no adapter/SDK/dispatcher edits):
1. **Discriminators + account order from official source.** Use the verified
   layouts in `docs/drift/audit-and-plan.md` §2 (sourced from `drift.json` IDL) and
   compute each discriminator as `sha256("global:<snake_case_name>")[..8]`:
   `initialize_user_stats`, `initialize_insurance_fund_stake`,
   `add_insurance_fund_stake`. Do not hardcode bytes blindly — compute in-harness.
2. **Pick a controlled authority.** For the smoke, use a fresh local keypair as the
   IF-stake `authority` (NOT the adapter PDA yet — that comes in the adapter step).
   Airdrop it lamports in litesvm.
3. **Derive the runtime PDAs** for that authority:
   `user_stats = ["user_stats", authority]`,
   `if_stake = ["insurance_fund_stake", authority, 0u16le]`.
4. **Stage token accounts.** Create/clone the authority's USDC ATA and fund it with
   USDC (mint USDC into it via `set_account`, or clone a funded account). Needed by
   `add_insurance_fund_stake.userTokenAccount`.
5. **Cloned program accounts** already staged in Layer B: state, drift_signer,
   spot_market[0], spot_market_vault[0], insurance_fund_vault[0], USDC mint — plus
   the **USDC oracle** (read SpotMarket[0].oracle and clone it).
6. **Invoke the minimal real IF path** in this order, asserting Drift logs each step:
   `initialize_user_stats` → `initialize_insurance_fund_stake(0)` →
   `add_insurance_fund_stake(0, amount)`. Success = `Program dRiftyHA… success`
   logs (not InstructionMissing), and a non-zero `if_shares` read back from the IF
   stake account. That is the scoped `[C] OK`.
7. (Deferred to the adapter step, not Layer C) `request_remove` → clock warp →
   `remove_insurance_fund_stake`, and switching `authority` to the adapter state PDA.

Open items to resolve before Layer C runs (no guessing):
- Exact account-struct offsets for `InsuranceFundStake.if_shares` and
  `SpotMarket.insurance_fund.{total_shares, unstaking_period}` — resolve from Drift
  on-chain Rust source or the full IDL `types` block (the fetched IDL truncated
  before `types`).
- Whether `add_insurance_fund_stake` requires the spot-market to be cloned in a
  state that passes Drift's internal IF invariants at the cloned slot (it has no
  oracle in its account list, which is favorable).

Layer C will be a separate, still-isolated harness path (extend
`tools/drift-litesvm-smoke` or a sibling bin) and will not touch production code.

## Layer C — real IF instruction sequence (BLOCKED)
`tools/drift-litesvm-smoke/src/main.rs` runs a dispatch preflight after Layer B and
before attempting Layer C when `DRIFT_SO` is set. The preflight first requires the
known-present `admin_withdraw_from_insurance_fund_vault` instruction to dispatch,
then requires `initialize_insurance_fund_stake` and `add_insurance_fund_stake` to
dispatch.

With a fresh mainnet dump of `fixtures/drift.so`, the known-present admin instruction
dispatches, but both required IF-stake instructions return Anchor 101
(`InstructionFallbackNotFound`). The harness therefore stops before Layer C with:

```text
LAYER_C_BLOCKED: dumped Drift binary lacks IF-stake instruction handlers
```

The fresh dump remained `6,673,069` bytes and has SHA-256
`72CC3062523BF87B028B5BEE163872140AD7A91F0EE32497E4159DAAA312D396`.
The deployed on-chain IDL was fetched through `@coral-xyz/anchor@0.28.0`
`Program.fetchIdl` because the `anchor` CLI was unavailable. It contains
`initializeUserStats`, `initializeInsuranceFundStake`, and
`addInsuranceFundStake`; its `BorshInstructionCoder` produces the same
discriminators already used by the harness.

This proves the freshly dumped deployed `.so` is real Drift BPF but does not
dispatch the handlers advertised by its on-chain IDL. Layer C is **BLOCKED by a
deployed-binary/on-chain-IDL mismatch**, not successful.

Key detail — **clock realignment**: Layers A/B warp the clock to ~`0+Nd` (epoch
~1.2M), but the cloned mainnet accounts hold real 2026 timestamps (~1.75e9). Layer C
resets `Clock.unix_timestamp` to `SpotMarket.insurance_fund.last_revenue_settle_ts`
(offset 392..400) so Drift's revenue-settle time math (`now - last_settle_ts`) does
not underflow. The 13/14-day cooldown warp matters only to the later `remove` step.

Run (host):
```powershell
cd D:\claudeCode\repo-hunter\solana-yield-adapter-standard\tools\drift-litesvm-smoke
$env:DRIFT_SO = "fixtures\drift.so"
$env:DRIFT_ACCOUNTS_DIR = "fixtures\accounts"
$env:WARP_DAYS = "14"
cargo run --release
```
Layer C success would require Drift `success` logs for `add_insurance_fund_stake`,
`if_shares > 0`, and a printed `[C] OK`. The current dumped binary cannot reach that
sequence because the dispatch preflight blocks first.

## Overall status
Gate-1 feasibility: PROVEN. Gate-2 smoke Layer A + Layer B: PASSED on host. Layer C
(real IF instruction): **BLOCKED by confirmed deployed-binary/on-chain-IDL
mismatch**. Drift IF adapter integration: NOT claimed.
