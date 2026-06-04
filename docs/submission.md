# Submission Notes

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

## Not Yet Claimable

Do not claim the full bounty requirements are complete yet.

Still required before a final bounty-grade submission:

1. Fund the devnet payer and deploy both programs.
2. Initialize the registry and register the five reference adapter configs on devnet.
3. Replace protocol account placeholders in `docs/protocol-adapters.md` and `scripts/clone-mainnet-accounts.ts`.
4. Implement real protocol CPI account maps for Kamino, MarginFi, Jupiter LP, Maple Syrup, and Drift Insurance Fund.
5. Run all five mainnet-fork tests and paste transaction/log evidence here.
