# Submission Notes

**Status: weak / not submitted — needs live verification before final claim.**
Repo pushed and source-complete; full bounty requirements (devnet deploy + 5 mainnet-fork tests) not yet met. Deadline 2026-06-09 (~5 days). Not yet submitted on Superteam Earn (HUMAN_ONLY listing, must be submitted by the human account holder).

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

Devnet deployment was prepared but not completed because the generated payer has 0 devnet SOL and CLI airdrops were rate-limited.

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
Maple syrupUSDC      PENDING — confirm Solana program id + mint from maple.finance (launched mid-2025)
```

Sources: Kamino KLend (explorer.solana.com / solscan), MarginFi v2 (docs.marginfi.com),
Drift v2 (docs.drift.trade program vault addresses), JLP mint (explorer.solana.com),
Maple syrupUSDC-on-Solana (maple.finance/insights).

## Verified in sandbox (2026-06-04)

```text
npm run build      : pass (tsc)
npm run typecheck  : pass (tsc --noEmit)
npx vitest run     : 11 passed, 7 skipped (SDK ABI + mainnet-fork readiness)
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

## Not Yet Claimable

Do not claim the full bounty requirements are complete yet.

Still required before a final bounty-grade submission (all on the Windows machine):

1. Fund the devnet payer and run `scripts/devnet-deploy.ps1` (deploy both programs).
2. Initialize the registry and register the five reference adapter configs on devnet.
3. Confirm Maple Solana program id + syrupUSDC mint (only remaining PENDING wiring).
4. Implement and compile real protocol CPI for Kamino, MarginFi, Jupiter LP, Maple Syrup,
   and Drift Insurance Fund (reference adapter currently uses a simulated virtual-yield model).
5. Run all five mainnet-fork tests via `npm run fork:run` flow and paste tx/log evidence here.
