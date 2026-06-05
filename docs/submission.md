# Submission Notes

**Status: partial live evidence / not submitted - Kamino and MarginFi USDC mainnet-fork roundtrips passed, but this is not a full bounty claim (not all five adapters; `CPI_IMPLEMENTED` stays false).**
Repo pushed and synced through `53bd145`. Devnet deploy is DONE, and the Kamino
USDC path now has deposit CPI, full-pool withdraw CPI, a real current-value
decoder, an official klend-sdk oracle fixture, and one local mainnet-fork
roundtrip pass. The full bounty is still not claimable until all five adapters
have live mainnet-fork roundtrip evidence. Deadline
2026-06-09 (~5 days). Not yet submitted on Superteam Earn (HUMAN_ONLY listing,
must be submitted by the human account holder).

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

## Current Snapshot (2026-06-05)

This section supersedes older Kamino status notes below.

Latest pushed `origin/main` commits checked before the local fork-evidence patch:

```text
53bd145 docs: update Kamino current value submission status
1a0dc0f test: add Kamino current value oracle fixture
82a2c2b feat: add Kamino current value decoder
15cfb00 feat: add Kamino full-pool withdraw CPI path
c551674 feat: add Kamino deposit CPI path
6af5809 chore: register devnet reference adapters
9272dca docs: update Kamino init CPI status
1b490f0 feat: add Kamino init CPI entrypoint
836501a feat: add Kamino CPI account layout gate scaffolding
4a8dfe3 test: validate Kamino CPI remaining account fixtures
e00074f test: add Kamino CPI account plan fixture checks
646957e chore: add Kamino CPI account plan fixture
```

Kamino implemented scope:

- `kamino_init`: real klend setup CPI for state-PDA-owned user metadata and
  obligation.
- `kamino_deposit`: real klend v2 deposit CPI, using the adapter USDC vault as
  `userSourceLiquidity` and validating the canonical remaining-account fixture.
  The older mainnet klend combined mutation instruction rejects CPI with
  `CpiDisabled`; the v2 mutation path is the live CPI path.
- `kamino_withdraw`: real klend v2 full-position/full-pool withdraw CPI using
  Kamino's `u64::MAX` collateral amount convention, redeeming into the adapter
  vault, enforcing `min_assets_out`, transferring USDC to the user, burning local
  shares, and zeroing pool totals.
- `current_value_cpi` on the Kamino route: read-only decoder for refreshed klend
  `Reserve` + `Obligation` account bytes; converts deposited cTokens to
  underlying USDC lamports and updates pooled `total_assets`.
- `tests/fixtures/kamino-current-value-424277911.json`: raw mainnet account
  fixture decoded with official `@kamino-finance/klend-sdk@3.2.26`.

Kamino guarded / not claimed:

- `CPI_IMPLEMENTED === false` remains intentional; the SDK must not advertise
  the full CPI matrix as complete.
- Partial Kamino withdraw still fails loudly with
  `KaminoPartialWithdrawUnsupported`; only full-pool withdraw is implemented.
- At fixture slot `424277911`, `KAMINO_USDC_OBLIGATION`
  (`BMVjGznYqketbFdniGVSjmghmUduqYvsspnqXvpz9Maa`) was not initialized on
  mainnet. The fixture records that caveat and uses a separate real initialized
  USDC Kamino obligation
  (`2HNipSa8PHXN77Pmymq22k8fnh2yT4p8snaofCkuDzqk`) only to prove the Rust decoder
  against official SDK math on real raw klend bytes.
- Live mainnet-fork evidence is scoped to Kamino USDC through the direct
  reference-adapter runner only. It is not a dispatcher/full-SDK claim and not an
  all-five-adapter pass.

Current verification evidence:

```text
cargo check --workspace: PASS
cargo test -p reference_yield_adapter kamino_decode_tests: PASS
cargo test -p reference_yield_adapter: PASS
npm exec tsx -- scripts/kamino-current-value-oracle.ts: PASS
npm run test -w @solana-yield-adapter-standard/sdk -- kamino-cpi-plan: PASS
npm run typecheck -w @solana-yield-adapter-standard/sdk: PASS
npx vitest run tests/mainnet-fork.spec.ts: PASS
node --check scripts\kamino-mainnet-fork-roundtrip.mjs: PASS
node scripts\kamino-mainnet-fork-roundtrip.mjs: PASS
```

Oracle fixture result:

```text
slot=424277911
sdkVersion=3.2.26
depositedCollateral=125715157
sdkExpectedAssets=149062854
rustAssets=149062854
diffLamports=0
```

Live mainnet-fork roundtrip status:

```text
PASS (local solana-test-validator mainnet fork, 2026-06-05)
runner=node scripts\kamino-mainnet-fork-roundtrip.mjs
forkSlot=424290277
sequence=initialize_adapter -> kamino_init -> initObligationFarmsForReserve
         -> refreshReserve + refreshObligation + kamino_deposit
         -> refreshReserve + refreshObligation + refreshObligationFarmsForReserve
         -> refreshReserve + refreshObligation + current_value_cpi
         -> refreshReserve + refreshObligation + kamino_withdraw
mutationPath=KLend v2 deposit/withdraw CPI
result=deposit, current_value, and full-pool withdraw passed
rounding=user recovered 999999 of 1000000 deposited USDC lamports; current value
         decoded 999999, matching the redeemed amount within 1 lamport.
```

Fork transaction evidence:

| Step | Signature | CU |
|---|---|---:|
| initializeAdapter | `2CuRRrUKhaVCL5HVCUSEw4oSUiwjybP4jE44GvTUC8gJQjMAV1KPbVhwv8kM34Vsy1VmT4EA7pMqpzEeLoN9mZJi` | 19,982 |
| kaminoInit | `4ysMQRhhvXMpbXFjcuXQ5whPVZCyC2tfnKo1nQLmJFB4iRqaQDREoobiYwSs26HPSRvrNCsXqT8CC2R8AUykFbPX` | 39,591 |
| initObligationFarm | `5NZHm8gUWPZo6zfvSzRCxugH4zzwfCn3v3pnFzNTpLRmsD9ZdK5ZdtFpBhYZCAqRuC3YfivNiZzqdz7QUGKNN2SW` | 51,526 |
| deposit | `3ATXrBLo66LrFrrqsBki1zjupbwE1DdzWUuV8gwHXYjjVQaa754E7JFz55EjE6H8YbM2sAhU3bgocEWXrHacJY8Q` | 187,542 |
| refreshFarmAfterDeposit | `P5sd9njP4zyYNro13ngLDuFdNfZwE99fToMQE9zWWYHqKtyQpPVtmhPpuwiaan5Z4ouvNuz3Dp8ZYVwVQBo6gbv` | 85,505 |
| currentValue | `Q6ANgQxZaY51Pa8yWbhpvzfwXNEvqRnuzmZMgh5taM9893KFtHssV3vKxeYpWbAyhbvaoBc8nxaXXjSj3zw9MZr` | 91,832 |
| withdraw | `3RKeCZECjn8wigLXxvCtHCZu2nM45ossBYXqv6iVrvyuenSuDCNGTZEQXxfKQ8jjhw7XMdE8MEazMq8DHWzHc7uH` | 182,645 |

Fork account evidence:

```text
user=GXFeYjgDfQzgCpGr5fbpVioWSshKjRRopoPwg7rmTRoF
state=4Y8QTjo8oBfnS3362aMkyiNawJV4ZStWrcm1PLZaoa7R bump=252
position=BdSNQwzH5xKL9CR61f4XgnoZ1rbGz61cTzA8aTjzHCn4
userUsdc=7qMiB33r4YsHXawsCdaf5oUWsFKtGjUisFEigy1gyvtS
adapterVault=Bw2mmBxrzTvba5q4y9kb5jUM57nu8Wv7h2EivmjkNt7R
obligation=BMVjGznYqketbFdniGVSjmghmUduqYvsspnqXvpz9Maa
obligationFarmState=FpvYH3vrip5Zaj5C2YPF6hGC17rPC5FLNEzt1ZPBmDzy
```

Fork deltas:

| Field | Before | After deposit | After current_value | After withdraw |
|---|---:|---:|---:|---:|
| user USDC | 2,000,000 | 1,000,000 | 1,000,000 | 1,999,999 |
| adapter vault USDC | 0 | 0 | 0 | 0 |
| reserve liquidity supply | 7,165,649,180,128 | 7,165,650,180,128 | 7,165,650,180,128 | 7,165,649,180,129 |
| reserve destination collateral | 90,365,882,072,908 | 90,365,882,916,271 | 90,365,882,916,271 | 90,365,882,072,908 |
| obligation collateral | null -> 0 after init | 843,363 | 843,363 | closed |
| adapter totalAssets | null -> 0 after init | 1,000,000 | 999,999 | 0 |
| adapter totalShares | null -> 0 after init | 1,000,000 | 1,000,000 | 0 |
| position shares | null | 1,000,000 | 1,000,000 | 0 |
| position lastValueAssets | null | 1,000,000 | 999,999 | 0 |

Key log assertions captured from the fork:

```text
deposit: Instruction: DepositReserveLiquidityAndObligationCollateralV2
deposit: Deposit reserve liquidity 1000000 and obligation collateral 843363
farm after deposit: stake 0 -> 843363
currentValue: Instruction: CurrentValueCpi
withdraw: Instruction: WithdrawObligationCollateralAndRedeemReserveCollateralV2
withdraw: Withdraw obligation collateral 843363 and redeem reserve collateral 999999
withdraw: Closing account
farm after withdraw: stake 843363 -> 0
```

## MarginFi USDC live mainnet-fork roundtrip (2026-06-05)

Scoped to the MarginFi USDC direct reference-adapter runner
`scripts/marginfi-mainnet-fork-roundtrip.mjs`. This is not a dispatcher/full-SDK
claim and not an all-five-adapter pass. `CPI_IMPLEMENTED` remains `false`.

Command (Windows PowerShell, from repo root):

```powershell
$env:MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com"
node scripts\marginfi-mainnet-fork-roundtrip.mjs
```

Live mainnet-fork roundtrip status:

```text
PASS / ROUNDTRIP_OK (local solana-test-validator mainnet fork, 2026-06-05)
runner=node scripts\marginfi-mainnet-fork-roundtrip.mjs
forkSlot=424380501
evidenceSlot=424351480
localRpc=http://127.0.0.1:8899
sequence=initialize_adapter -> marginfi_init -> marginfi_deposit
         -> current_value_cpi -> marginfi_withdraw
mutationPath=MarginFi lending_account_deposit / lending_account_withdraw CPI
sbf=target\sbf-solana-solana\release\reference_yield_adapter.so (607104 bytes, 2026-06-05 10:37)
result=deposit, current_value, and full withdraw passed
rounding=user fully recovered 1000000 of 1000000 deposited USDC lamports
         (finalUserDeltaVsStart=0); current_value decoded 999999, within 1
         lamport of deposited principal.
```

Fork transaction evidence:

| Step | Signature | CU |
|---|---|---:|
| initializeAdapter | `n61AXttuhJYbw2fao9g4KnwXdA1Ju8J1HUqgDkk6X5aGVCmE4W2U8fN8P6mrkGPDbjb7qzGNjhAwHNjhkd9xJL3` | 14,009 |
| marginfiInit | `4UUR9W4Df3WR3TRArmj6GuxcfpmCunuNTFfiQ51gRBQYzY155RUQtnXS554HMovybZv98XNVV8VAJ3LsrwtoHstp` | 15,261 |
| deposit | `3TXGZ3qRc25YcyD5VhTwzEqQS43nV8c8XTaw7D3fonnNoR7x6md5GtwhgBf4fHdjcDguWMgDQWeATxCM7LEK7tDn` | 158,546 |
| currentValue | `2ATbLeT1SE8JhPJQrNFqNQ89edRdZ4QeKTg54gRF2bqWzGUkGKRPVYh5L6cf2DEDwKCzvWR3BKMBqVcctxNGDpPz` | 25,049 |
| withdraw | `4tL7VRmVomTNqv6SAJx9uy24nmgYZgu4zy8vm1cwAzQ6Uxm1DByyhmTzS3kSho9NeuXN9iikSWDUQ9jgKsZfZecC` | 152,024 |

Fork account evidence:

```text
user=2Tbze5pPWetSJpzEouTkw2n4PFxnvMN6tTn5tCkaspkb
state=CfG2iD5YizACMXNsLbfmkwym51EM59RrP99xX5yZADvg
position=7vZjaJfRCj8cmTJo3J6WKkCi6qocNaHYh4WNs9bhiWBw
userUsdc=BZzF2GhpqqvnRSCz5s3AM7T5HQytmyKFzg7bkk7duNwP
adapterVault=2xg8GdS1KsRRCotGj3Ziy9KYX8wDqbjdERHo7YB6bN66
marginfiAccount=8Q7ABWcW8ZD959CYx8DLhGGFGxY4NYgkvgFQQkqeEt65
depositAmount=1000000
```

Fork deltas:

| Field | Before | After deposit | After current_value | After withdraw |
|---|---:|---:|---:|---:|
| user USDC | 2,000,000 | 1,000,000 | 1,000,000 | 2,000,000 |
| bank liquidity vault | 390,125,657,535 | 390,126,657,535 | 390,126,657,535 | 390,125,657,535 |
| adapter totalAssets | 0 (after init) | 1,000,000 | 999,999 | 0 |
| adapter totalShares | 0 (after init) | 1,000,000 | 1,000,000 | 0 |
| position shares | - | 1,000,000 | 1,000,000 | 0 |

`redeemedToUser=1000000`, `finalUserDeltaVsStart=0`.

Notes captured from the fork:

```text
- Deposit reduced user USDC by exactly 1000000 (2000000 -> 1000000) and added
  1000000 to the MarginFi USDC bank liquidity vault.
- current_value_cpi is read-only against MarginFi: it set adapter totalAssets to
  999999 (1-lamport rounding under principal) without moving funds.
- Full withdraw redeemed 1000000 back to the user (vault delta restored to
  390125657535) and zeroed adapter totals and position shares.
- The fresh signer-backed MarginFi account (8Q7ABWcW8ZD959CYx8DLhGGFGxY4NYgkvgFQQkqeEt65)
  was created during marginfi_init and reused read-only by current_value and as
  the marginfi_account meta in deposit/withdraw.
- Live program logs confirmed Anchor dispatch entered MarginfiInit,
  MarginfiDeposit, CurrentValueCpi, and MarginfiWithdraw. A simple Node byte scan
  for raw discriminators returning -1 is not a failure signal; on-chain dispatch
  is the source of truth.
- Post-success `ws ECONNREFUSED 127.0.0.1:8900` lines are harmless; they occur
  after ROUNDTRIP_OK when the validator is intentionally killed.
```

Generic CPI route audit:

- Generic non-Kamino `deposit_cpi`, `withdraw_cpi`, and `current_value_cpi` still
  fail loudly by design: no remaining accounts -> `MissingCpiAccounts`; remaining
  accounts on unsupported adapters -> `CpiNotImplemented`; no simulated fallback
  exists.
- Kamino-specific real paths now present: `kamino_init`, `kamino_deposit`,
  full-pool `kamino_withdraw`, and read-only real `current_value_cpi`.
- MarginFi-specific real paths now present and live-fork verified (2026-06-05):
  `marginfi_init`, `marginfi_deposit`, full `marginfi_withdraw`, and read-only
  real `current_value_cpi`.
- Jupiter LP, Maple Syrup, and Drift Insurance Fund still do not have real
  protocol CPI implementations. Their generic CPI routes remain loud-fail only.
- Maple Syrup remains a Chainlink CCIP / token-route integration, not a native
  lending CPI path.

Final bounty submission checklist:

- Rebuild SBF/IDL artifacts from pushed commit `1a0dc0f` or newer.
- Start a mainnet-fork validator with the static clone list plus derived Kamino
  reserve, obligation, user metadata, farm, Scope, token, and sysvar accounts.
- Run the Kamino split sequence: refreshes, `kamino_init` if accounts are empty,
  `kamino_deposit`, real value refresh/readback, and full-pool `kamino_withdraw`.
- For the fork run, remember that the adapter-derived Kamino obligation was not
  initialized at snapshot slot `424277911`; initialize it via `kamino_init` before
  expecting the adapter's live `current_value_cpi` to read it.
- Capture fork slot, tx signatures, program logs, compute budget, user/vault USDC
  deltas, obligation collateral deltas, and adapter state/position fields.
- Implement and verify the remaining four protocol integrations before claiming
  all five adapters pass.
- Submit only from the human Superteam account holder, after confirming regional
  eligibility and the listing is still open.

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

Note: devnet deployment and registry registration are complete. All-adapter
CPI/live mainnet-fork roundtrip validation is still not claimed as complete;
only the scoped Kamino USDC direct-reference-adapter fork pass is recorded.

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
still `PENDING`, and keeps fake live passes out of CI. The scoped Kamino live
roundtrip is driven by `scripts/kamino-mainnet-fork-roundtrip.mjs`; the older
`scripts/run-mainnet-fork.mjs` remains a guided all-adapter preflight flow.

## Runnable now vs still open

Runnable now:
- Real, verified program ids/mints wired; fork clone command generates cleanly.
- Fork-readiness suite green; SDK ABI green.
- Devnet deployment is DONE; the earlier payer/faucet blocker is historical only
  and superseded by the deployment evidence above.
- Devnet registry initialization and the five reference adapter registrations are
  DONE; evidence is recorded above.
- Kamino USDC direct-reference-adapter live mainnet-fork roundtrip passes at slot
  `424290277` for init -> deposit -> current_value -> full-pool withdraw.
- Turnkey scripts remain available: `scripts/devnet-deploy.ps1`,
  `scripts/devnet-register-reference-adapters.ts`, `scripts/run-mainnet-fork.mjs`,
  and `scripts/kamino-mainnet-fork-roundtrip.mjs`.

Still open:
- Run live mainnet-fork roundtrips for MarginFi, Jupiter LP, Maple Syrup, and
  Drift Insurance Fund after their real integrations are implemented.
- Kamino partial-withdraw collateral conversion remains intentionally guarded;
  the recorded Kamino pass is full-pool withdraw only.
- Finish MarginFi / Jupiter / Maple / Drift real CPI paths.

## Adapter interface prepared for real CPI (2026-06-04)

The reference adapter now exposes the plumbing required for real protocol CPI.
After `1b490f0`, Kamino had the first-deposit setup CPI entrypoint; after
`c551674`, Kamino deposit CPI was pushed; after `15cfb00`, full-pool withdraw
CPI was pushed; after `82a2c2b` and `1a0dc0f`, real current-value decoding and
the official SDK oracle fixture were pushed.

- Added `anchor-spl` (token feature) and a new `AdapterCpiRoute` account context
  carrying the user underlying token account, an adapter-side vault/receipt token
  account, `token_program`, and `remaining_accounts` for protocol-specific accounts.
- Added separate `deposit_cpi` / `withdraw_cpi` / `current_value_cpi` instructions.
  Unsupported routes fail LOUDLY (`MissingCpiAccounts` if protocol accounts are
  absent, otherwise `CpiNotImplemented`). They never fall back to simulated yield.
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

Kamino real CPI now has one scoped live mainnet-fork pass for
`kamino_deposit`, real `current_value_cpi`, and full-pool `kamino_withdraw`.
The current-value decoder also matches the official SDK oracle fixture with
`diffLamports=0`. Partial-withdraw collateral conversion remains intentionally
guarded, and MarginFi / Jupiter / Maple / Drift real CPI paths are still open.
This is still NOT a full live-CPI bounty submission.

## Kamino USDC CPI status (2026-06-04): init + deposit + full-pool withdraw + value decoder

Commit `1b490f0` implements the Rust `kamino_init` entrypoint. It performs the
first-deposit setup path:

- `initUserMetadata`
- `initObligation`

The init CPI is PDA-signed by the adapter state PDA with signer seeds
`[b"adapter", adapter_id, &[state.bump]]`. It moves no funds and does not
implement real value refresh/readback.

Commit `c551674` adds `kamino_deposit`. Commit `15cfb00` adds full-position /
full-pool `kamino_withdraw` using Kamino's `u64::MAX` withdraw convention.
Commit `82a2c2b` adds the current-value decoder, and commit `1a0dc0f` adds the
official klend-sdk oracle fixture. The withdraw path intentionally rejects
partial withdraws until proportional collateral redemption is implemented.

The approved Kamino transaction shape remains split transaction: refresh
instructions stay as top-level sibling klend instructions built by the
dispatcher/client, and only PDA-signed mutation paths belong inside the adapter.

Kamino live evidence status after the 2026-06-05 fork run:

1. Live fork roundtrip is recorded above at fork slot `424290277`.
2. The adapter-derived obligation was initialized on the fork via `kamino_init`.
3. The proof is intentionally scoped to full-pool withdraw.
4. Partial-withdraw collateral conversion remains guarded and is not claimed.

Kamino CPI status: init + deposit + full-pool withdraw + current-value decoder
are implemented, with SDK oracle fixture evidence for the decoder and local
mainnet-fork evidence for the direct reference-adapter full-pool flow. MarginFi /
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

Kamino `kamino_init` CPI is implemented in `1b490f0`; the adapter-derived
metadata/obligation are initialized by the live fork runner before deposit. A
Kamino deposit -> current_value -> full-pool withdraw roundtrip passed locally on
2026-06-05, but the full bounty remains NOT claimable.

Update 2026-06-04 (fix): the first run returned BLOCKED because the script called
`KaminoMarket.load` without the required `recentSlotDurationMs` argument and selected
the reserve by the symbol "USDC". The script now (a) calls `load(connection, market,
recentSlotDurationMs, programId, false, true)` per the installed klend-sdk types,
(b) enumerates every reserve via `market.getReserves()`, (c) selects the USDC reserve
strictly by underlying mint (EPjFW...G8mm), and (d) on no-match returns BLOCKED with a
full `reserveEnumeration` of what the market actually contains. Lending-market authority
uses `market.getLendingMarketAuthority()`. Verified here by `tsc` against the installed
SDK types; the live `npm run kamino:derive` must be run on the Windows machine
when refreshing the account map. This derivation note is historical; the later
Kamino fork runner used the derived map above and passed deposit/value/withdraw.
The full bounty is still not claimable.

## Not Yet Claimable

Do not claim the full bounty requirements are complete yet.

Still required before a final bounty-grade submission (all on the Windows machine):

1. Devnet deploy DONE — both programs deployed and confirmed on devnet (see "Devnet deployment evidence").
2. Devnet registry DONE — registry initialized and all five reference adapter configs registered.
3. Maple addresses are resolved (mint/router/pool/oracle wired). The Maple integration
   path is a Chainlink CCIP / token route, not a lending CPI — the live flow is still TODO.
4. Finish and compile the remaining real protocol integrations for MarginFi,
   Jupiter LP, Maple Syrup (CCIP route), and Drift Insurance Fund. Kamino init,
   deposit, full-pool withdraw, current-value decoding, SDK oracle evidence, and
   one scoped live mainnet-fork roundtrip are recorded, but partial-withdraw
   collateral conversion remains intentionally guarded.
5. Run all five mainnet-fork tests via `npm run fork:run` flow and paste tx/log evidence here.
