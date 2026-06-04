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

Current local command:

```bash
npx vitest run tests/mainnet-fork.spec.ts
```

Expected readiness result after the Kamino withdraw update:

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
The live roundtrip remains gated behind `MAINNET_FORK_LIVE=1` and should be driven
through `scripts/run-mainnet-fork.mjs` against a real `solana-test-validator`
fork. Once real account maps and protocol CPI clients are wired, replace the live
placeholder with generated Anchor client calls and keep the default readiness
checks active in CI.
