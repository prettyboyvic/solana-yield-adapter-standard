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

For mainnet-fork preparation:

```bash
MAINNET_RPC_URL=https://your-mainnet-rpc npm run fork:accounts
```

## Program IDs

Local/devnet IDs are pre-filled so the repo is deterministic:

- Dispatcher: `CP1io1rppTn7HDg8qzpGh629KG6K23ipcgtVZjWMtXmw`
- Reference adapter: `CjGjc5uAnEuXBfRc9NKiNTxcvjxZA9snZ3V1MqKvJpoY`

## Documentation

- [Adapter standard](docs/standard.md)
- [Build your own adapter](docs/build-your-own-adapter.md)
- [Mainnet-fork tests](docs/mainnet-fork-tests.md)
- [Protocol adapter notes](docs/protocol-adapters.md)
- [Submission notes](docs/submission.md)

