# Solana Yield Adapter Standard

Reference implementation for the Superteam Earn bounty "Develop Solana Yield Adapter Standard".

The repository defines:

- Anchor dispatcher program with `deposit`, `withdraw`, and `current_value` routing.
- Governance-gated on-chain adapter registry.
- Anchor-compatible adapter ABI and TypeScript SDK encoders.
- Reference adapter program with five protocol modes:
  - Kamino USDC
  - MarginFi USDC
  - Jupiter LP
  - Maple Syrup
  - Drift Insurance Fund
- Mainnet-fork test harness and protocol account-map checklist.
- Markdown standard and "build your own adapter" guide.

## Status

This workspace is ready for source review and SDK verification, but it is not a
full bounty claim yet.

Current pushed Kamino USDC coverage:

- Real klend setup, deposit CPI, and full-pool withdraw CPI paths.
- Real read-only current-value decoder for refreshed Kamino reserve/obligation
  bytes.
- Oracle fixture at `tests/fixtures/kamino-current-value-424277911.json`
  cross-checking the Rust decoder against
  `@kamino-finance/klend-sdk@3.2.26`; result: `diffLamports=0`.

Guarded / still not claimed:

- `CPI_IMPLEMENTED` remains `false`; the SDK does not advertise full CPI
  completion.
- No live mainnet-fork deposit -> current_value -> withdraw transaction
  signatures are included yet.
- At fixture slot `424277911`, the adapter-derived Kamino obligation was not
  initialized on mainnet; the fixture records that caveat and uses a separate
  initialized USDC obligation only for decoder proof.
- Partial Kamino withdraw and the other four protocol real-CPI paths remain
  loud-fail / not implemented.

A full bounty-grade submission still requires live mainnet-fork evidence for all
five adapters against a pinned RPC snapshot.

## Quick Start

```bash
npm install
npm test
npm run build
```

With the Solana toolchain installed:

```bash
anchor build
anchor test
```

If `cargo-build-sbf` wrapper detection fails on Windows, the programs can still be compiled with the Solana platform Cargo:

```bash
cargo build --release --target sbf-solana-solana --workspace
```

For mainnet-fork preparation:

```bash
MAINNET_RPC_URL=https://your-mainnet-rpc npm run fork:accounts
```

## Program IDs

Local/devnet IDs are pre-filled from generated deploy keypairs:

- Dispatcher: `37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY`
- Reference adapter: `BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1`

## Documentation

- [Adapter standard](docs/standard.md)
- [Build your own adapter](docs/build-your-own-adapter.md)
- [Mainnet-fork tests](docs/mainnet-fork-tests.md)
- [Protocol adapter notes](docs/protocol-adapters.md)
- [Submission notes](docs/submission.md)
