# Mainnet-Fork Tests

The bounty requires all five adapters to pass against mainnet state. This repository provides the harness shape; the concrete protocol account maps must be filled before the tests can be honestly enabled.

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

`tests/mainnet-fork.spec.ts` is skipped unless `MAINNET_RPC_URL` is set. Once real account maps are wired, remove the explicit throw in each test body and call the generated Anchor clients with the configured accounts.

