# Submission Notes

**Status: weak / not submitted — needs live verification before final claim.**
Repo pushed; source is not bounty-complete. Devnet deploy is DONE, but 5 mainnet-fork (real CPI) tests are not yet met. Deadline 2026-06-09 (~5 days). Not yet submitted on Superteam Earn (HUMAN_ONLY listing, must be submitted by the human account holder).

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

### Devnet registry initialization and adapter registration evidence

Verified on 2026-06-04 with `npm run devnet:register`.

Registry PDA: `AwzSLAgzqc7cS1aAJdKdLFQGCbjWExPvFpoeboRhcsvA`

Registry fields decoded from devnet:

- Governance: `H9AgiD3PKeHEuELfifBw4PqUfnsWyt5L5ruKPin89n6d`
- Version: `1`
- Adapter count / max: `5 / 5`
- Metadata URI: `ipfs://solana-yield-adapters/registry.json`
- All five records active, with adapter program
  `BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1`

Transaction signatures:

- `initialize_registry`: `XRWcwmrvW9VKs5WY5yLjjpgZZaB9cYBbRpttmWYjtNTaPfvUjLKLe9Pwy4bG1Hns2QDREG1LNecThfeDPGDp3DM`
- `register_adapter:kamino-usdc`: `kG6o7rUSP6gHbMe5FRu8HsPu2zWc5noggjHc3wGzyALBkX5HNrACkqvCYSsngy98VzcCtESC41sTC2EmF8ee259`
- `register_adapter:marginfi-usdc`: `JfkT1W9SUFRm1Fhqza3Tggxh4PnjQiAmbi4N6WYVVyDbzGy9wccpVgdPvoGGX15LmfQUwa2oph1EqKroZUZmGAX`
- `register_adapter:jupiter-lp`: `462rKgK2BgcWz43DpHG26jDuep4Uv4X3hta4Asa8ecbLwzLNQjUGaYFhBR91iQ9KCwYSKzHutV2q2p4cauPMUkdX`
- `register_adapter:maple-syrup`: `43PDs6iFKrGN22J9Ja7DKY35PoLKP252byJLkCH3fjCXPrTr9j3W5EjpxkqDc5qDSMssswbQZS9Lh4ukYa9rBqbs`
- `register_adapter:drift-insurance-fund`: `34sb9ed7nj7cBz8bU7FEFtam5cpg7eMg3fEc1rRqrmkoAvRYJuZUMANax7crCTqUqvsrdNoEUPbcAqtw6eD1Sqcs`

Registered adapter records:

| Adapter | Protocol | Underlying mint | Receipt mint | Capabilities | Risk tier |
|---|---:|---|---|---:|---:|
| `kamino-usdc` | 1 | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | `11111111111111111111111111111111` | 7 | 2 |
| `marginfi-usdc` | 2 | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | `11111111111111111111111111111111` | 7 | 2 |
| `jupiter-lp` | 3 | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | `27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4` | 7 | 3 |
| `maple-syrup` | 4 | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | `AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj` | 7 | 3 |
| `drift-insurance-fund` | 5 | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | `11111111111111111111111111111111` | 7 | 4 |

The default pubkey receipt mint records are intentional placeholders for adapters
whose receipt/account state is derived on-machine rather than a static SPL mint.

Note: devnet deployment and registry registration are complete. CPI/live
mainnet-fork roundtrip validation is still not claimed as complete.

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
USDC mint            EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
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
- Devnet deployment is DONE; the earlier payer/faucet blocker is historical only
  and superseded by the deployment evidence above.
- Devnet registry initialization and the five reference adapter registrations are
  DONE; evidence is recorded above.
- Turnkey scripts remain available: `scripts/devnet-deploy.ps1`,
  `scripts/devnet-register-reference-adapters.ts`, `scripts/run-mainnet-fork.mjs`.

Still open:
- Run the live mainnet-fork roundtrip with real per-protocol CPI.
- Finish Kamino `kamino_deposit`, `kamino_withdraw`, and real `current_value`.
- Finish MarginFi / Jupiter / Maple / Drift real CPI paths.

## Adapter interface prepared for real CPI (2026-06-04)

The reference adapter now exposes the plumbing required for real protocol CPI.
After `1b490f0`, Kamino also has the first-deposit setup CPI entrypoint only:

- Added `anchor-spl` (token feature) and a new `AdapterCpiRoute` account context
  carrying the user underlying token account, an adapter-side vault/receipt token
  account, `token_program`, and `remaining_accounts` for protocol-specific accounts.
- Added separate `deposit_cpi` / `withdraw_cpi` / `current_value_cpi` instructions.
  Their bodies fail LOUDLY (`MissingCpiAccounts` if protocol accounts are absent,
  otherwise `CpiNotImplemented`). They never fall back to simulated yield.
- Added `kamino_init` in commit `1b490f0`; it performs only the first-deposit
  Kamino setup path (`initUserMetadata` + `initObligation`) using the state PDA
  signer seeds and moves no funds.
- The existing `deposit` / `withdraw` / `current_value` instructions are unchanged
  and now explicitly labelled SIMULATED REFERENCE ONLY (virtual-yield accounting,
  not bounty-grade CPI).
- Rust unit tests (`cpi_route_tests`) assert the routing decision is loud and has
  no success/simulated branch. SDK tests assert the CPI route has distinct
  discriminators from the simulated route and that `CPI_IMPLEMENTED === false`.

Historical note: the original scaffold was not compiled in the earlier sandbox
environment. The Kamino init CPI patch in `1b490f0` was later verified before
commit with `cargo check -p reference_yield_adapter`, `cargo test -p
reference_yield_adapter`, SDK typecheck/tests, `npm run kamino:cpi-plan`, and
`git diff --check`.

Kamino full real CPI is still incomplete: `kamino_deposit`, `kamino_withdraw`, and
real `current_value` are not implemented or passing. MarginFi / Jupiter / Maple /
Drift real CPI paths are still open. This is still NOT a full live-CPI bounty
submission.

## Kamino USDC CPI status (2026-06-04): init CPI only

Commit `1b490f0` implements the Rust `kamino_init` entrypoint. It performs the
first-deposit setup path only:

- `initUserMetadata`
- `initObligation`

The init CPI is PDA-signed by the adapter state PDA with signer seeds
`[b"adapter", adapter_id, &[state.bump]]`. It moves no funds, does not deposit,
does not withdraw, and does not implement real value refresh/readback.

The approved Kamino transaction shape remains split transaction: refresh
instructions stay as top-level sibling klend instructions built by the
dispatcher/client, and only PDA-signed mutation paths belong inside the adapter.

Still required before claiming Kamino passes:

1. Implement `kamino_deposit`.
2. Implement `kamino_withdraw`.
3. Implement real Kamino `current_value`.
4. Pass a mainnet-fork deposit -> current_value -> withdraw roundtrip and paste tx
   signatures, slots, balance deltas, and logs here.

Kamino CPI status: init CPI only, not a passing real-CPI adapter. MarginFi /
Jupiter / Maple / Drift: still open. This is NOT a claim that all five adapters
pass, and NOT full bounty completion.

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

Kamino `kamino_init` CPI is implemented in `1b490f0`, but no mainnet-fork deposit
-> current_value -> withdraw roundtrip has passed and the full bounty remains NOT
claimable.

Update 2026-06-04 (fix): the first run returned BLOCKED because the script called
`KaminoMarket.load` without the required `recentSlotDurationMs` argument and selected
the reserve by the symbol "USDC". The script now (a) calls `load(connection, market,
recentSlotDurationMs, programId, false, true)` per the installed klend-sdk types,
(b) enumerates every reserve via `market.getReserves()`, (c) selects the USDC reserve
strictly by underlying mint (EPjFW...G8mm), and (d) on no-match returns BLOCKED with a
full `reserveEnumeration` of what the market actually contains. Lending-market authority
uses `market.getLendingMarketAuthority()`. Verified here by `tsc` against the installed
SDK types; the live `npm run kamino:derive` must be run on the Windows machine (this build
environment has no mainnet RPC). Still no deposit/withdraw/value CPI roundtrip, bounty
not claimable.

## Not Yet Claimable

Do not claim the full bounty requirements are complete yet.

Still required before a final bounty-grade submission (all on the Windows machine):

1. Devnet deploy DONE — both programs deployed and confirmed on devnet (see "Devnet deployment evidence").
2. Devnet registry DONE — registry initialized and all five reference adapter configs registered.
3. Maple addresses are resolved (mint/router/pool/oracle wired). The Maple integration
   path is a Chainlink CCIP / token route, not a lending CPI — the live flow is still TODO.
4. Finish and compile real protocol integration for Kamino, MarginFi, Jupiter LP,
   Maple Syrup (CCIP route), and Drift Insurance Fund. Kamino init CPI is done in
   `1b490f0`, but Kamino deposit/withdraw/value and the other adapters' real CPI
   paths are still open.
5. Run all five mainnet-fork tests via `npm run fork:run` flow and paste tx/log evidence here.
