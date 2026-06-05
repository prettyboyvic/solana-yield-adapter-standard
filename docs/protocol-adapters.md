# Protocol Adapter Notes

This document records the account maps that must be finalized before claiming the bounty's "all five adapters pass mainnet-fork tests" requirement.

## Shared Rules

Every protocol adapter must:

- Keep the dispatcher ABI unchanged.
- Validate protocol market accounts against adapter state.
- Use slippage floors from dispatcher input.
- Update the user's deterministic position PDA.
- Emit deposit, withdraw, and value events.
- Use a pinned fork slot in test evidence.

## Kamino USDC

Yield type: USDC lending/vault position.

Required account map:

```text
user signer
adapter state PDA
position PDA
user USDC token account
adapter USDC token account or authority PDA
Kamino market account
Kamino reserve account
Kamino obligation/account state
Kamino lending program
token program
system program
```

### Verified klend CPI flow (from official klend IDL / @kamino-finance/klend-sdk)

Kamino lending (program `KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD`) uses an
obligation-based deposit/withdraw flow. Each user action is a small instruction
sequence; state-changing CPIs must be preceded by refreshes:

Deposit (USDC -> cToken collateral in obligation):
1. `refresh_reserve` (USDC reserve)
2. `refresh_obligation` (lending market + obligation + touched reserves)
3. `deposit_reserve_liquidity_and_obligation_collateral`, accounts (per klend IDL):
   owner(signer), obligation, lending_market, lending_market_authority(PDA),
   reserve, reserve_liquidity_supply(vault), reserve_collateral_mint,
   reserve_destination_deposit_collateral(cToken vault), user_source_liquidity(USDC ATA),
   user_destination_collateral(placeholder/none), collateral_token_program,
   liquidity_token_program, instruction_sysvar.

Withdraw (cToken collateral -> USDC):
1. `refresh_reserve` + `refresh_obligation`
2. `withdraw_obligation_collateral_and_redeem_reserve_liquidity`, accounts (per klend IDL):
   owner(signer), obligation, lending_market, lending_market_authority(PDA), reserve,
   reserve_source_collateral(cToken vault), reserve_collateral_mint, reserve_liquidity_supply,
   user_destination_liquidity(USDC ATA), user_destination_collateral(placeholder),
   collateral_token_program, liquidity_token_program, instruction_sysvar.

Current value:
- `refresh_reserve` + `refresh_obligation`, then read `obligation.deposits[].market_value`
  (or convert collateral->liquidity via the refreshed reserve collateral exchange rate).

Per-market addresses that MUST be derived on-machine via klend-sdk for the Main Market
(do NOT hardcode unverified): lending_market, lending_market_authority(PDA), USDC reserve,
reserve_liquidity_supply, reserve_collateral_mint, reserve_destination_deposit_collateral,
the user obligation PDA, and the exact oracle accounts referenced by the reserve config.
Exact instruction discriminators + per-account writable/signer flags must come from the
pinned klend IDL version in use.

Open items:

- Derive Main-Market USDC reserve + its sub-accounts via `@kamino-finance/klend-sdk`
  (`market.getReserve('USDC')`), not by pasting addresses.
- Implement the three CPI sequences above in `reference_yield_adapter` (Kamino branch).
- Run the dispatcher-level deposit -> current value -> withdraw mainnet-fork test.

## MarginFi USDC

Yield type: USDC bank deposit.

Required account map:

```text
user signer
adapter state PDA
position PDA
user USDC token account
adapter USDC token account or authority PDA
marginfi group
marginfi bank
marginfi account
marginfi program
token program
system program
```

Open items:

- Replace `MARGINFI_RECEIPT_MINT_REPLACE_WITH_MAINNET`.
- Pin bank and group accounts.
- Implement deposit and withdraw CPI wrappers.

## Jupiter LP

Yield type: USDC deposit into Jupiter Perps JLP.

Pinned mainnet identities:

```text
Jupiter Perps program: PERPHjGBqRHArX4DySjwM6UJHiR3sWAatqfdBS2qQJu
JLP mint:              27G8MtK7VtTcCHkpASjSDdkWWYfoqT6ggEuKidVJidD4
JLP pool:              5BUwFW4nRbftYTDMbgxykoFWqWHPzahFSNAaaaJtVKsq
USDC custody:          G18jKKXQwBbrHeiK3C9MRXhkHsLHf7XgCSisykV46EZa
USDC custody vault:    WzWUoCmtVv7eqAbU3BfKPU3fhLP6CXR8NCJH78UK9VS
```

`docs/jupiter-derived-accounts.json` derives and verifies these accounts from
the deployed Jupiter Perps Anchor IDL plus current mainnet Pool/Custody state.
`packages/sdk/fixtures/jupiter-cpi-account-plan.json` pins the executable plan.

Mutation account shape:

1. The 14 formal `addLiquidity2` / `removeLiquidity2` IDL accounts.
2. Ten undocumented AUM remaining accounts: all five pool custodies followed by
   their five Doves AG price accounts.
3. The state PDA signs as Jupiter owner, owns the adapter USDC/JLP ATAs, and
   records actual minted/burned JLP lamports as shares.

The formal IDL field named `custodyDovesPriceAccount` resolves to the USDC
Custody's current `dovesAgOracle`, not its legacy `dovesOracle`.

`current_value_cpi` is read-only against `[pool, JLP mint, adapter JLP ATA]`:

```text
assets = floor(adapterJlpAmount * pool.aumUsd / jlpMintSupply)
```

Implemented and verified:

- Real `jupiter_deposit` and `jupiter_withdraw` CPI paths.
- Raw mainnet current-value fixture at slot `424386975`.
- Scoped direct-adapter mainnet-fork roundtrip at slot `424386975`.

Fork caveat: Jupiter's Doves AG feeds have very short freshness limits, while
the Windows validator needs roughly 30 seconds to load/JIT the 10 MB Perps
program and its warped clock later jumps forward. The runner therefore loads raw
mainnet Doves AG bytes with only `publish_time` forwarded and records every
before/after timestamp in its evidence JSON.

## Maple Syrup

Gate 1 verdict: **REQUIRES DESIGN CHANGE**. See
[`docs/maple/gate1-feasibility.md`](maple/gate1-feasibility.md).

Yield type: syrupUSDC exposure carried as a Chainlink CCIP/CCT token route, not
a native Solana lending receipt.

Verified Solana account map:

```text
syrupUSDC mint         AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj
CCIP Router            Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C
CCIP pool config       HrTBpF3LqSxXnjnYdR4htnBLyMHNZ6eNaDZGPundvHbm
CCIP pool program      787uwTCd8b2ikQP6g9AapMky36PWDv9x1XpC5ZUAfDYc
syrupUSDC/USDC oracle  CpNyiFt84q66665Kx64bobxZuMgZ2EecrhAJs1HikS2T
Whirlpool candidate    6fteKNvMdv7tYmBoJHhj1jx6rHcEwC6RdSEmVpyS613J
```

The CCIP pool-config account is a token-route configuration PDA, not a Maple
lending pool. Maple native mint/redeem sends cross-chain messages to Ethereum
and settles asynchronously, so it cannot satisfy the current adapter ABI's
atomic `deposit` / `withdraw` and minimum-output guarantees.

A Solana DEX route may be technically possible, but it would be a separate
market-execution design for buying and selling Maple exposure, not native Maple
mint/redeem. A receipt-token custody route would require changing the adapter
underlying from USDC to syrupUSDC. Until an explicit design is selected, Maple
mutation routes remain loud-fail and `CPI_IMPLEMENTED=false`.

## Drift Insurance Fund

Yield type: Drift insurance fund stake.

Required account map:

```text
user signer
adapter state PDA
position PDA
Drift state
Drift spot market
insurance fund stake account
insurance fund vault
oracle
Drift program
token program
system program
```

Open items:

- Replace `DRIFT_IF_SHARES_REPLACE_WITH_MAINNET`.
- Pin market index and oracle.
- Implement add/remove insurance fund stake CPI wrappers.

## Mainnet-Fork Completion Criteria

For each adapter:

1. Clone all protocol accounts at a pinned slot.
2. Fund or impersonate a test user with the underlying asset.
3. Run deposit with a nonzero slippage floor.
4. Run current value and assert the value field is nonzero.
5. Run withdraw and assert shares decrease.
6. Save transaction signatures/log output in `docs/submission.md`.
