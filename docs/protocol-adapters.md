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

Yield type: LP or JLP-like receipt position.

Required account map:

```text
user signer
adapter state PDA
position PDA
user underlying token account
adapter token account or authority PDA
Jupiter pool/state account
Jupiter vault accounts
Jupiter program
token program
system program
```

Open items:

- Replace `JUPITER_LP_UNDERLYING_REPLACE_WITH_MAINNET`.
- Replace `JUPITER_LP_RECEIPT_REPLACE_WITH_MAINNET`.
- Define fair value source for `current_value`.

## Maple Syrup

Yield type: Syrup/Maple yield receipt.

Required account map:

```text
user signer
adapter state PDA
position PDA
user underlying token account
adapter token account or authority PDA
Maple pool
Maple receipt mint/account
Maple program
token program
system program
```

Open items:

- Replace `MAPLE_SYRUP_RECEIPT_REPLACE_WITH_MAINNET`.
- Pin pool and receipt accounts.
- Define redemption and share price logic.

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

