# Solana Yield Adapter Standard

Version: `SYAS-1`

## Goals

The standard gives Solana yield protocols one minimal integration shape:

- A dispatcher program routes user actions to approved adapters.
- A registry stores governance-approved adapter metadata.
- Every adapter exposes the same Anchor instruction names and argument layout.
- Adapter-specific protocol accounts are passed as remaining accounts so the dispatcher stays protocol-neutral.

## Dispatcher Interface

The dispatcher exposes three user-facing instructions:

```rust
deposit(adapter_id: [u8; 32], amount: u64, min_shares_out: u64)
withdraw(adapter_id: [u8; 32], shares: u64, min_assets_out: u64)
current_value(adapter_id: [u8; 32])
```

The dispatcher:

1. Loads the registry PDA at seed `["registry"]`.
2. Finds `adapter_id`.
3. Checks that the adapter is active and supports the requested capability.
4. Checks that the provided adapter program matches the registered program id.
5. Builds an Anchor-compatible CPI payload and invokes the adapter.

## Adapter ABI

An adapter must expose these Anchor instructions:

```rust
deposit(adapter_id: [u8; 32], amount: u64, min_shares_out: u64)
withdraw(adapter_id: [u8; 32], shares: u64, min_assets_out: u64)
current_value(adapter_id: [u8; 32])
```

Instruction data is:

```text
8 bytes  Anchor discriminator sha256("global:<method>")[0..8]
32 bytes adapter_id
8 bytes  u64 little-endian amount/shares, except current_value
8 bytes  u64 little-endian slippage floor, except current_value
```

The canonical account order for adapter CPIs is:

```text
0. user signer, read-only or writable depending on adapter needs
1. adapter state PDA
2. user position PDA
3. system/token/protocol accounts in adapter-defined order
```

The reference adapter uses:

```text
user signer
adapter state PDA: ["adapter", adapter_id]
position PDA: ["position", adapter_id, user]
system program
```

Protocol adapters extend the same order with token accounts, vaults, markets, oracles, and protocol programs after the position PDA.

## Registry

The registry is initialized once by governance:

```rust
initialize_registry(max_adapters: u16, metadata_uri: String)
```

Governance can:

- `register_adapter(adapter_id, config)`
- `set_adapter_status(adapter_id, active)`
- `set_governance(new_governance)`

The registry record stores:

- `adapter_id`
- `program_id`
- `protocol`
- `underlying_mint`
- `receipt_mint`
- `authority`
- `capabilities`
- `risk_tier`
- `active`
- `metadata_uri`
- `metadata_hash`

Capability bits:

```text
1 << 0 deposit
1 << 1 withdraw
1 << 2 current_value
```

## Semantics

### deposit

`deposit` moves or accounts for `amount` units of the adapter's underlying asset. The adapter returns shares by minting, transferring, or updating a position account. It must fail if fewer than `min_shares_out` shares would be credited.

### withdraw

`withdraw` redeems `shares` from the user's adapter position. It must fail if fewer than `min_assets_out` underlying units would be returned.

### current_value

Solana CPI cannot synchronously return arbitrary data to the dispatcher. The standard therefore requires `current_value` to update a deterministic value account or position field and emit an event. The reference adapter updates `Position.last_value_assets` and emits `AdapterValue`.

## Adapter IDs

Adapter IDs are 32-byte identifiers. The SDK derives reference IDs as:

```text
sha256("solana-yield-adapter:<label>")
```

Protocol teams may choose another deterministic derivation if the final 32-byte value is unique in the registry.

## Safety Requirements

Adapters should:

- Verify `adapter_id` against their state PDA.
- Check the user signer owns the position.
- Enforce slippage floors.
- Keep share and asset math in `u128` before casting to `u64`.
- Emit events for every state-changing operation.
- Pause through adapter authority and registry governance independently.
- Treat `current_value` as a state update, not a return value.

