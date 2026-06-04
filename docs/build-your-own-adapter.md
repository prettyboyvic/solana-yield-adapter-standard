# Build Your Own Adapter

This guide targets an Anchor team that wants to ship a new adapter in less than one day.

## 1. Choose the Yield Primitive

Pick one protocol position type:

- Lending deposit
- LP share
- Vault receipt
- Insurance fund stake
- Tokenized RWA/yield receipt

Define the underlying asset mint, receipt/share mint, market account, and oracle/value source.

## 2. Create Adapter State

Use a PDA keyed by adapter id:

```rust
#[account]
pub struct AdapterState {
    pub adapter_id: [u8; 32],
    pub authority: Pubkey,
    pub underlying_mint: Pubkey,
    pub receipt_mint: Pubkey,
    pub protocol_market: Pubkey,
    pub value_oracle: Pubkey,
    pub paused: bool,
    pub bump: u8,
}
```

Seed:

```text
["adapter", adapter_id]
```

## 3. Create Position State

Use a PDA keyed by adapter id and user:

```rust
#[account]
pub struct Position {
    pub owner: Pubkey,
    pub adapter_id: [u8; 32],
    pub shares: u64,
    pub principal_assets: u64,
    pub last_value_assets: u64,
    pub bump: u8,
}
```

Seed:

```text
["position", adapter_id, owner]
```

## 4. Implement the Required Instructions

Your Anchor program must expose these exact method names:

```rust
pub fn deposit(ctx: Context<AdapterRoute>, adapter_id: [u8; 32], amount: u64, min_shares_out: u64) -> Result<()>
pub fn withdraw(ctx: Context<AdapterRoute>, adapter_id: [u8; 32], shares: u64, min_assets_out: u64) -> Result<()>
pub fn current_value(ctx: Context<AdapterRoute>, adapter_id: [u8; 32]) -> Result<()>
```

Use this account order:

```rust
#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct AdapterRoute<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [b"adapter", adapter_id.as_ref()], bump = state.bump)]
    pub state: Account<'info, AdapterState>,
    #[account(mut, seeds = [b"position", adapter_id.as_ref(), user.key().as_ref()], bump = position.bump)]
    pub position: Account<'info, Position>,
    // protocol-specific accounts follow
}
```

## 5. Wire Protocol CPI

For a real protocol adapter:

1. Validate all protocol accounts against known market/config accounts.
2. Transfer user assets into the adapter vault or protocol deposit account.
3. Invoke the protocol deposit/withdraw instruction by CPI.
4. Compute shares from protocol receipt balances, not from user input.
5. Update `Position.shares` and `Position.last_value_assets`.

For `current_value`, read the protocol's share price, exchange rate, or oracle and write the value into the position.

## 6. Register Through Governance

The registry governance registers the adapter with:

```rust
register_adapter(adapter_id, AdapterRegistration {
    protocol,
    underlying_mint,
    receipt_mint,
    authority,
    capabilities,
    risk_tier,
    metadata_uri,
    metadata_hash,
})
```

The adapter is callable only after registration and while active.

## 7. Test Checklist

Minimum tests:

- Initializes adapter state.
- Rejects wrong adapter id.
- Rejects wrong owner.
- Rejects slippage failure.
- Deposits and credits shares.
- Refreshes `current_value`.
- Withdraws and debits shares.
- Pauses and rejects calls.
- Runs the same cases against a mainnet-fork account set.

## 8. Submission Checklist

- Public GitHub repository.
- `anchor build` output.
- Devnet deployment address.
- Mainnet-fork logs for every supported adapter.
- Markdown spec and adapter guide.

