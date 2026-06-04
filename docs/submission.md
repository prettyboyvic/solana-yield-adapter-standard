# Submission Notes

**Status: weak / not submitted — needs live verification before final claim.**
Repo pushed and source-complete; full bounty requirements: devnet deploy DONE; 5 mainnet-fork (real CPI) tests not yet met. Deadline 2026-06-09 (~5 days). Not yet submitted on Superteam Earn (HUMAN_ONLY listing, must be submitted by the human account holder).

Superteam Earn listing checked on 2026-06-04:

- Title: Develop Solana Yield Adapter Standard
- Sponsor: Superteam Ukraine
- Status: OPEN
- Reward: 1,000 USDC total, split 700 / 300
- Deadline: 2026-06-09T20:59:59.999Z
- Public page: https://superteam.fun/earn/listing/develop-solana-yield-adapter-standard
- Submission count shown on page: 0
- Submission requirement: public GitHub repository link
- Important eligibility warning: the page marks the listing as regional and only open for people in Ukraine.
- Agent warning: API details show `agentAccess = HUMAN_ONLY`, so final submission should be done from a human Superteam account.

## Public Repository

Public GitHub repository:

```text
https://github.com/prettyboyvic/solana-yield-adapter-standard
```

## Program IDs

Generated devnet/local program keypairs are stored under ignored `target/deploy/`.

```text
yield_adapter_dispatcher: 37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY
reference_yield_adapter: BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1
devnet payer: H9AgiD3PKeHEuELfifBw4PqUfnsWyt5L5ruKPin89n6d
```

## Local Verification

Ran on 2026-06-04:

```text
cargo check --quiet: pass
npm test: pass, 5 SDK ABI tests
npm run build: pass
npm audit --audit-level=high: pass, 0 vulnerabilities
anchor idl build -p yield_adapter_dispatcher: pass, IDL printed to stdout
anchor idl build -p reference_yield_adapter: pass, IDL printed to stdout
Solana platform Cargo SBF build: pass
```

SBF command used on Windows with Agave/Solana 2.2.20 platform tools:

```powershell
$sol = "C:\Users\vudat\.local\share\solana\install\releases\2.2.20\solana-release\bin"
$pt = "$sol\platform-tools-sdk\sbf\dependencies\platform-tools\rust\bin"
& "$pt\cargo.exe" build --release --target sbf-solana-solana --workspace
```

Generated artifacts:

```text
target/sbf-solana-solana/release/yield_adapter_dispatcher.so
target/sbf-solana-solana/release/reference_yield_adapter.so
```

## Devnet Deploy Status

(Superseded — payer was later funded and both programs are now deployed on devnet; see "Devnet deployment evidence" below. History retained.)

Devnet deployment was initially prepared but not completed because the generated payer had 0 devnet SOL and CLI airdrops were rate-limited.

Attempts made on 2026-06-04:

```text
solana airdrop 5: failed, rate limit
solana airdrop 2: failed, rate limit
solana airdrop 1: failed, rate limit
solana airdrop 0.2: failed, rate limit
```

Deploy attempt confirmed the blocker:

```text
dispatcher deploy requires about 2.54 SOL plus fee from payer H9AgiD3PKeHEuELfifBw4PqUfnsWyt5L5ruKPin89n6d
```

After the payer is funded with devnet SOL, deploy with:

```powershell
$sol = "C:\Users\vudat\.local\share\solana\install\releases\2.2.20\solana-release\bin"
& "$sol\solana.exe" program deploy target\sbf-solana-solana\release\yield_adapter_dispatcher.so --program-id target\deploy\yield_adapter_dispatcher-keypair.json --keypair target\deploy\devnet-payer.json --url https://api.devnet.solana.com
& "$sol\solana.exe" program deploy target\sbf-solana-solana\release\reference_yield_adapter.so --program-id target\deploy\reference_yield_adapter-keypair.json --keypair target\deploy\devnet-payer.json --url https://api.devnet.solana.com
```

Recommended funding target: at least 6 devnet SOL for both programs plus fees.

## Devnet deployment evidence

Devnet payer: `H9AgiD3PKeHEuELfifBw4PqUfnsWyt5L5ruKPin89n6d`

### Deployed programs

- Dispatcher program: `37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY`
  - Deploy signature: `46r8w7D7N5gsLhUS1NpKux5U7fgqrdrrsZm2j8eKoqDLb7VaackCy3CXfAfTveDJ1NqMGV8jw1zX2xv3EAu6Awsg`
  - ProgramData address: `71vFTFR1d5KJACwM198RJDceDUPp4t2xdi9afDkaWvRQ`
  - Upgrade authority: `H9AgiD3PKeHEuELfifBw4PqUfnsWyt5L5ruKPin89n6d`
  - Last deployed slot: `467004096`
  - Data length: `364760 bytes`

- Reference yield adapter program: `BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1`
  - Deploy signature: `2JSC3B7tU47PJCURRNDcZ7gZ3dkyCyxitNxEd5Q1GiCwynTkZkCippch55SL79VsddgepiGyyNf3GwY6KMx2axCt`
  - ProgramData address: `EN3xzxdLt8DVz8bAEf3rZ5dgRYGjLAytJhd8uay6ppL8`
  - Upgrade authority: `H9AgiD3PKeHEuELfifBw4PqUfnsWyt5L5ruKPin89n6d`
  - Last deployed slot: `467004121`
  - Data length: `359368 bytes`

Both programs were confirmed on devnet with `solana program show --url https://api.devnet.solana.com`.

### Registered reference adapters printed by deploy script

The deploy script printed five reference adapter records:

- `kamino-usdc`
- `marginfi-usdc`
- `jupiter-lp`
- `maple-syrup`
- `drift-insurance-fund`

The printed adapter program for all five records is:

`BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1`

Note: devnet deployment is complete. CPI/live mainnet-fork roundtrip validation is still not claimed as complete.

## Known Toolchain Issue

`anchor build --no-idl` still fails on this Windows setup before compilation:

```text
The Solana toolchain is corrupted. Please, run cargo-build-sbf with the --force-tools-install argument to fix it.
```

`cargo-build-sbf --force-tools-install` was already run. The underlying platform `cargo.exe` and `rustc.exe` work, and direct SBF build succeeds. This appears to be a Windows wrapper/toolchain detection issue, not a program compilation issue.

## Mainnet Wiring (verified 2026-06-04)

Replaced the previous `*_REPLACE` placeholders with web-verified, stable mainnet ids
in `packages/sdk/src/index.ts`. Per-market accounts (reserves, banks, groups,
obligations, IF stake) are intentionally NOT hardcoded — they are derived on-machine
via each protocol SDK and marked `DERIVE_VIA_PROTOCOL_SDK_ON_MACHINE`.

```text
USDC mint            EPjFWdd5AufqSSqeM2qzH6oEgCG1kduA3s3z2nZ7G8mm
Kamino KLend program KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD
MarginFi v2 program  MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA
Drift v2 program     dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH   (corrected; old notes had a wrong tail)
Jupiter JLP mint     27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4
Maple syrupUSDC mint AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj
Maple CCIP Router    Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C
Maple CCIP Pool      HrTBpF3LqSxXnjnYdR4htnBLyMHNZ6eNaDZGPundvHbm
Maple syrupUSDC/USDC oracle  CpNyiFt84q66665Kx64bobxZuMgZ2EecrhAJs1HikS2T
```

Maple address PENDING is now resolved. Note: Maple syrupUSDC on Solana is a
**Chainlink CCIP / token route, not a native Solana lending CPI** — so the maple-syrup
adapter's live integration and mainnet-fork roundtrip are still NOT complete. Only the
addresses are wired; no CPI was implemented in this patch.

Sources: Kamino KLend (explorer.solana.com / solscan), MarginFi v2 (docs.marginfi.com),
Drift v2 (docs.drift.trade program vault addresses), JLP mint (explorer.solana.com),
Maple syrupUSDC + CCIP router/pool/oracle (official Maple docs, maple.finance).

## Verified in sandbox (2026-06-04)

```text
npm run build      : pass (tsc)
npm run typecheck  : pass (tsc --noEmit)
npx vitest run     : 15 passed, 7 skipped (SDK ABI + CPI-route separation + fork readiness)
fork:accounts      : prints a complete solana-test-validator --clone command (5 static accounts)
fork:run           : preflight + ordered on-machine instructions
```

`tests/mainnet-fork.spec.ts` no longer throws unconditionally: it now runs
fork-READINESS checks (wiring coherence + clone-account set) in CI, skips protocols
still `PENDING`, and defers the live on-validator roundtrip to `scripts/run-mainnet-fork.mjs`.

## Runnable now vs still open

Runnable now (this commit):
- Real, verified program ids/mints wired; fork clone command generates cleanly.
- Fork-readiness suite green; SDK ABI green.
- Turnkey scripts: `scripts/devnet-deploy.ps1`, `scripts/run-mainnet-fork.mjs`.

Cannot be done from this environment (no Solana toolchain, no devnet/mainnet network,
no validator) — must run on the Windows machine with Agave/Anchor 2.2.20:
- Devnet deploy (payer still needs >= 6 devnet SOL; faucet was rate-limited).
- Live mainnet-fork roundtrip with real per-protocol CPI.

## Adapter interface prepared for real CPI (2026-06-04)

The reference adapter now exposes the plumbing required for real protocol CPI,
WITHOUT implementing any protocol integration yet:

- Added `anchor-spl` (token feature) and a new `AdapterCpiRoute` account context
  carrying the user underlying token account, an adapter-side vault/receipt token
  account, `token_program`, and `remaining_accounts` for protocol-specific accounts.
- Added separate `deposit_cpi` / `withdraw_cpi` / `current_value_cpi` instructions.
  Their bodies fail LOUDLY (`MissingCpiAccounts` if protocol accounts are absent,
  otherwise `CpiNotImplemented`). They never fall back to simulated yield.
- The existing `deposit` / `withdraw` / `current_value` instructions are unchanged
  and now explicitly labelled SIMULATED REFERENCE ONLY (virtual-yield accounting,
  not bounty-grade CPI).
- Rust unit tests (`cpi_route_tests`) assert the routing decision is loud and has
  no success/simulated branch. SDK tests assert the CPI route has distinct
  discriminators from the simulated route and that `CPI_IMPLEMENTED === false`.

Note: the Rust changes (anchor-spl dep + new context/instructions) were NOT compiled
in this environment (no Rust toolchain). `cargo check --quiet` and `cargo test` must be
run on the Windows machine to confirm; anchor-spl transitive crates may need version
pins consistent with the existing Solana 2.2.20 platform-tools pins.

No protocol CPI (Kamino / MarginFi / Jupiter / Maple / Drift) is implemented.
This is interface scaffolding only and is still NOT a full live-CPI bounty submission.

## Kamino USDC CPI status (2026-06-04): BLOCKED in this environment

Attempted the first real protocol CPI (Kamino USDC). Result: NOT implemented and
NOT passing, blocked by environment limits. No CPI code was faked and no virtual-yield
fallback was added for Kamino.

Exact blockers (all required to produce a verified Kamino roundtrip):
1. No Rust toolchain available here -> cannot `cargo check` / `cargo test` an
   implementation, so any klend CPI written here would be uncompiled and unverifiable.
2. No mainnet RPC access here -> cannot derive the Main-Market USDC reserve sub-accounts
   (reserve_liquidity_supply, reserve_collateral_mint, reserve_destination_deposit_collateral,
   lending_market_authority PDA, obligation PDA, oracle accounts) via @kamino-finance/klend-sdk,
   and cannot read the reserve account on-chain. Per the no-guessing rule these must not be
   hardcoded from memory.
3. No local validator -> cannot run the deposit -> current value -> withdraw mainnet-fork
   roundtrip, which is required before claiming Kamino passes.

What WAS done (verified, sourced): the klend obligation-based deposit/withdraw/value
instruction sequences and their account maps are documented in `docs/protocol-adapters.md`
from the official klend IDL / @kamino-finance/klend-sdk. The real-CPI interface from
commit 8671b2b still fails loudly and does not fall back to simulation.

To finish Kamino on the Windows machine (has Rust/Anchor + RPC + validator):
derive the Main-Market USDC reserve accounts via klend-sdk, implement the three CPI
sequences in the Kamino branch of `reference_yield_adapter`, then run
`anchor test` / the mainnet-fork roundtrip and paste tx signatures + logs here.

Kamino CPI: BLOCKED (not passing). MarginFi / Jupiter / Maple / Drift: still open.
This is NOT a claim that all five adapters pass, and NOT full bounty completion.

### Kamino account derivation step (prepared 2026-06-04)

Added `scripts/derive-kamino-accounts.ts` (`npm run kamino:derive`) that uses the
official `@kamino-finance/klend-sdk` to load the Main Market
(`7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF`) and print the exact USDC account
map (reserve, liquidity supply vault, collateral mint, destination deposit collateral,
lending-market authority PDA, oracle accounts, obligation PDA strategy, token/ATA/
system/sysvar programs). Anything not verifiable on-chain is emitted as explicit
`null`/`BLOCKED` — no addresses are guessed.

Not run in the build environment (no mainnet RPC + packages not installed there).
Run locally on Windows:

```powershell
npm install
$env:KAMINO_RPC_URL="https://<your-mainnet-rpc>"
npm run kamino:derive
# optional: $env:OWNER="<adapter authority pubkey>"  # also derives obligation PDA
```

Kamino CPI is still NOT implemented, no mainnet-fork roundtrip has been run, and the
full bounty remains NOT claimable.

Update 2026-06-04 (fix): the first run returned BLOCKED because the script called
`KaminoMarket.load` without the required `recentSlotDurationMs` argument and selected
the reserve by the symbol "USDC". The script now (a) calls `load(connection, market,
recentSlotDurationMs, programId, false, true)` per the installed klend-sdk types,
(b) enumerates every reserve via `market.getReserves()`, (c) selects the USDC reserve
strictly by underlying mint (EPjFW...G8mm), and (d) on no-match returns BLOCKED with a
full `reserveEnumeration` of what the market actually contains. Lending-market authority
uses `market.getLendingMarketAuthority()`. Verified here by `tsc` against the installed
SDK types; the live `npm run kamino:derive` must be run on the Windows machine (this build
environment has no mainnet RPC). Still no CPI, no roundtrip, bounty not claimable.

## Not Yet Claimable

Do not claim the full bounty requirements are complete yet.

Still required before a final bounty-grade submission (all on the Windows machine):

1. Devnet deploy DONE — both programs deployed and confirmed on devnet (see "Devnet deployment evidence").
2. Initialize the registry and register the five reference adapter configs on devnet.
3. Maple addresses are resolved (mint/router/pool/oracle wired). The Maple integration
   path is a Chainlink CCIP / token route, not a lending CPI — the live flow is still TODO.
4. Implement and compile real protocol integration for Kamino, MarginFi, Jupiter LP,
   Maple Syrup (CCIP route), and Drift Insurance Fund (reference adapter currently uses
   a simulated virtual-yield model). No CPI was added in this patch.
5. Run all five mainnet-fork tests via `npm run fork:run` flow and paste tx/log evidence here.
