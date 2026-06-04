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

This workspace is ready for source review and SDK verification. A full bounty-grade submission still requires:

- Anchor 0.31.1, Solana 2.2.20, Rust, and a funded devnet deploy key.
- Real protocol CPI account maps for the five adapters.
- Mainnet-fork run evidence against a pinned RPC snapshot.
- Devnet deployment address for the registry program.

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
