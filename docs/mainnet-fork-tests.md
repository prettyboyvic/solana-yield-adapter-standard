# Mainnet-Fork Tests

The bounty requires all five adapters to pass against mainnet state. This
repository now has scoped Kamino USDC, MarginFi USDC, and Jupiter LP mainnet-fork
roundtrip passes; Maple Syrup and Drift Insurance Fund still need real
integrations and live evidence before a full bounty claim.

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

## MarginFi Live-Fork Preflight

The scoped MarginFi preflight validates the committed fork account map against
the real raw USDC bank fixture and the official
`@mrgnlabs/marginfi-client-v2@6.4.2` IDL decoder. It prints the exact
`solana-test-validator` command, but deliberately does not start a validator or
send transactions.

Windows PowerShell command:

```powershell
$env:MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com"
node scripts\marginfi-mainnet-fork-preflight.mjs
```

The account map pins the MarginFi program, USDC mint, production group, USDC
bank, bank liquidity vault, and bank oracle. The adapter state, adapter vault,
and liquidity-vault authority are deterministic derived accounts. The user,
user USDC account, position PDA, and fresh signer-backed MarginFi account remain
runtime-created accounts. Slot `424351480` is the raw-byte account-map evidence
slot; the preflight queries the RPC's current finalized slot for the validator
warp so cloned oracle data is not paired with a stale clock.

## MarginFi Live Runner

The scoped MarginFi runner (`scripts/marginfi-mainnet-fork-roundtrip.mjs`) starts
its own pinned `solana-test-validator` fork, clones the committed MarginFi
program plus the USDC bank/group/liquidity-vault/oracle from the account map,
loads the local SBF artifacts, creates local user/vault token fixtures, generates
a fresh signer-backed MarginFi account, and runs the sequence:

1. `initialize_adapter` (value_oracle = bank oracle);
2. `marginfi_init` with the fresh MarginFi account signer;
3. `marginfi_deposit`;
4. read-only `current_value_cpi` over `[usdc_bank, marginfi_account]`;
5. full `marginfi_withdraw` with health accounts `[usdc_bank, bank_oracle]`.

Windows command sequence used for the passing run:

```powershell
$sol = "C:\Users\vudat\.local\share\solana\install\releases\2.2.20\solana-release\bin"
$pt = "$sol\platform-tools-sdk\sbf\dependencies\platform-tools\rust\bin"
$env:PATH = "$pt;$env:PATH"
$env:RUSTC = "$pt\rustc.exe"
& "$pt\cargo.exe" build --release --target sbf-solana-solana --workspace
$env:MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com"
node scripts\marginfi-mainnet-fork-roundtrip.mjs
```

Passing evidence captured on 2026-06-05:

```text
forkSlot=424380501  evidenceSlot=424351480
ROUNDTRIP_OK
deposit -> current_value -> withdraw: PASS
user USDC: 2000000 -> 1000000 -> 1000000 -> 2000000
bank liquidity vault: 390125657535 -> 390126657535 -> 390126657535 -> 390125657535
adapter totalAssets: 0 -> 1000000 -> 999999 -> 0
adapter totalShares: 0 -> 1000000 -> 1000000 -> 0
position shares: 1000000 -> 0
redeemedToUser=1000000  finalUserDeltaVsStart=0
```

Full transaction signatures, compute units, and account fields are recorded in
`docs/submission.md`. This live runner is scoped to MarginFi USDC only;
`CPI_IMPLEMENTED` remains `false` and it is not an all-five-adapter pass. The
preflight above remains an account-map readiness gate that does not start a
validator.

## Jupiter LP Live Runner

The Jupiter account map is derived from the deployed Perps IDL and committed
mainnet state. The scoped runner (`scripts/jupiter-mainnet-fork-roundtrip.mjs`)
loads that plan plus the local adapter SBF, clones the JLP pool/USDC custody, and
runs:

1. `initialize_adapter`;
2. `jupiter_deposit` -> Jupiter `addLiquidity2`;
3. read-only `current_value_cpi`;
4. full `jupiter_withdraw` -> Jupiter `removeLiquidity2`.

Jupiter's 14-account IDL omits the remaining accounts used by its AUM
calculation. The committed plan appends all five pool Custodies followed by all
five Doves AG price accounts, for a 24-account Jupiter mutation CPI.

Windows command sequence used for the passing run:

```powershell
$sol = "C:\Users\vudat\.local\share\solana\install\releases\2.2.20\solana-release\bin"
$pt = "$sol\platform-tools-sdk\sbf\dependencies\platform-tools\rust\bin"
$env:PATH = "$pt;$env:PATH"
$env:RUSTC = "$pt\rustc.exe"
& "$pt\cargo.exe" build --release --target sbf-solana-solana --workspace
$env:MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com"
$env:JUPITER_FORK_SLOT = "424386975"
node scripts\jupiter-mainnet-fork-roundtrip.mjs
```

Passing evidence captured on 2026-06-05:

```text
forkSlot=424386975
ROUNDTRIP_OK
user USDC: 2000000 -> 1000000 -> 1995637
adapter JLP: 0 -> 295604 -> 0
adapter totalAssets: 0 -> 997522 -> 997522 -> 0
adapter totalShares: 0 -> 295604 -> 295604 -> 0
position shares: 295604 -> 295604 -> 0
redeemedToUser=995637  finalUserDeltaVsStart=-4363
```

Fork-specific oracle caveat: Doves AG freshness is shorter than the time needed
for the Windows validator to load/JIT the 10 MB Perps executable, and the warped
validator clock later jumps forward. The runner therefore loads the five Doves
AG accounts from raw mainnet bytes with only their `i64 publish_time` at offset
`177` forwarded by `60000` seconds. Prices, account owner, and all other bytes
remain unchanged. Every observed/forwarded timestamp is written to
`target/jupiter-mainnet-fork-fixtures/evidence.json`; this is not claimed as an
untouched-oracle snapshot.

Full transaction signatures, compute units, account deltas, state fields, and
the oracle-fixture caveat are recorded in `docs/submission.md`.

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
- Jupiter's formal IDL layout, pool-wide AUM suffix, Doves AG selection, adapter
  vaults, and current-value formula are pinned by committed fixtures/tests.

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
