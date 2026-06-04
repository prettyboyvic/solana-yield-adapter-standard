# Mainnet-Fork Tests

The bounty requires all five adapters to pass against mainnet state. This
repository now has one scoped Kamino USDC live mainnet-fork roundtrip pass; the
remaining four adapters still need real integrations and live evidence before a
full bounty claim.

## Required Toolchain

- Anchor 0.31.1
- Solana 2.2.20
- Rust stable compatible with Anchor 0.31.1
- Node 24 or later
- A mainnet RPC URL that supports cloning accounts

## Fork Flow

1. Fill protocol account maps in `docs/protocol-adapters.md`.
2. Add those accounts to `scripts/clone-mainnet-accounts.ts`.
3. Start a validator:

```bash
MAINNET_RPC_URL=https://your-mainnet-rpc npm run fork:accounts
```

The script prints a `solana-test-validator --clone ...` command.

4. In another terminal:

```bash
anchor build
anchor test --skip-local-validator
```

## Kamino Live Runner

The scoped Kamino runner starts its own pinned `solana-test-validator` fork,
loads the local SBF artifacts, creates local user/vault token fixtures, and runs
the split Kamino sequence:

1. top-level KLend refresh instructions;
2. `kamino_init` when the adapter-derived user metadata / obligation are empty;
3. `kamino_deposit`;
4. top-level refresh plus `current_value_cpi`;
5. full-pool `kamino_withdraw`.

Windows command sequence used for the passing run:

```powershell
$sol = "C:\Users\vudat\.local\share\solana\install\releases\2.2.20\solana-release\bin"
$pt = "$sol\platform-tools-sdk\sbf\dependencies\platform-tools\rust\bin"
$env:PATH = "$pt;$env:PATH"
$env:RUSTC = "$pt\rustc.exe"
& "$pt\cargo.exe" build --release --target sbf-solana-solana --workspace
$env:MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com"
node scripts\kamino-mainnet-fork-roundtrip.mjs
```

The runner clone/program set includes:

```text
--clone-upgradeable-program KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD
--clone-upgradeable-program FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr
--bpf-program BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1 target/sbf-solana-solana/release/reference_yield_adapter.so
--bpf-program 37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY target/sbf-solana-solana/release/yield_adapter_dispatcher.so
--clone EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v  # USDC mint
--clone 7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF  # lending market
--clone 9DrvZvyWh1HuAoZxvYWMvkf2XCzryCpGgHqrMjyDWpmo  # lending market authority
--clone D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59  # USDC reserve
--clone Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6  # reserve liquidity supply vault
--clone B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D  # reserve collateral mint
--clone 3DzjXRfxRm6iejfyyMynR4tScddaanrePJ1NJU2XnPPL  # reserve destination collateral
--clone 3t4JZcueEzTbVP6kLxXrL3VpWx45jDer4eqysweBchNH  # Scope price feed
--clone JAvnB9AKtgPsTEoKmn24Bq64UMoYcrtWtq42HHBdsPkh  # reserve collateral farm state
--account <generated user SOL>
--account <generated user USDC ATA>
--account <generated adapter vault USDC ATA>
```

Token programs and sysvars are provided by the local validator; the adapter-
derived user metadata, obligation, and obligation farm state are created during
the sequence.

Passing evidence captured on 2026-06-05:

```text
forkSlot=424290277
deposit -> current_value -> withdraw: PASS
user USDC: 2000000 -> 1000000 -> 1999999
Kamino collateral: 0 -> 843363 -> closed
adapter totalAssets: 0 -> 1000000 -> 999999 -> 0
current value matched the redeemed amount within 1 lamport
```

Full transaction signatures, compute units, account deltas, state fields, and
log excerpts are recorded in `docs/submission.md`.

## Current Readiness Coverage

The default Vitest fork suite is an offline readiness gate, not a fake live pass.
It currently checks:

- clone-account output is non-empty, deduped, and contains no unresolved sentinel
  values;
- each non-`PENDING` adapter has coherent static mainnet wiring;
- the committed Kamino CPI fixture contains every split-transaction section needed
  by the fork runner (`init`, refreshes, deposit, withdraw);
- Kamino withdraw account order is pinned to the klend IDL shape and redeems into
  the adapter vault.

Current local command:

```bash
npx vitest run tests/mainnet-fork.spec.ts
```

Expected readiness result after the Kamino v2 fork-runner update:

```text
1 file passed
8 tests passed
6 tests skipped
```

## Evidence Format

Paste logs into `docs/submission.md`:

```text
adapter: kamino-usdc
slot: <fork slot>
deposit: pass
current_value: pass
withdraw: pass
tx signatures:
- <signature>
```

Repeat for all five adapters.

## Enabling the Test File

`tests/mainnet-fork.spec.ts` always runs the offline readiness checks above.
The default readiness suite still avoids fake live passes. The scoped Kamino
live roundtrip is driven by `scripts/kamino-mainnet-fork-roundtrip.mjs` against
a real `solana-test-validator` fork. The older `scripts/run-mainnet-fork.mjs`
remains a guided preflight flow for the broader all-adapter fork effort.
