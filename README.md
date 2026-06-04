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

This workspace is ready for source review and SDK verification, and it now has
one Kamino USDC live mainnet-fork roundtrip pass. It is still not a full bounty
claim.

Current pushed Kamino USDC coverage:

- Real klend setup, deposit CPI, and full-pool withdraw CPI paths. The live fork
  runner uses KLend v2 mutation instructions because mainnet KLend rejects the
  older combined mutation instructions when called via CPI.
- Real read-only current-value decoder for refreshed Kamino reserve/obligation
  bytes.
- Oracle fixture at `tests/fixtures/kamino-current-value-424277911.json`
  cross-checking the Rust decoder against
  `@kamino-finance/klend-sdk@3.2.26`; result: `diffLamports=0`.
- Local mainnet-fork roundtrip at fork slot `424290277`: `kamino_init`,
  `kamino_deposit`, refreshed `current_value_cpi`, and full-pool
  `kamino_withdraw` passed. Evidence is recorded in
  `docs/submission.md`.

Guarded / still not claimed:

- `CPI_IMPLEMENTED` remains `false`; the SDK does not advertise full CPI
  completion.
- The live fork evidence is Kamino-only and direct-reference-adapter scoped; it
  is not an all-five-adapter pass.
- At fixture slot `424277911`, the adapter-derived Kamino obligation was not
  initialized on mainnet; the fixture records that caveat and uses a separate
  initialized USDC obligation only for decoder proof.
- Partial Kamino withdraw and the other four protocol real-CPI paths remain
  loud-fail / not implemented.

A full bounty-grade submission still requires the remaining four protocol
integrations and live mainnet-fork evidence for all five adapters against a
pinned RPC snapshot.

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

For the scoped Kamino live fork runner on Windows:

```powershell
$sol = "C:\Users\vudat\.local\share\solana\install\releases\2.2.20\solana-release\bin"
$pt = "$sol\platform-tools-sdk\sbf\dependencies\platform-tools\rust\bin"
$env:PATH = "$pt;$env:PATH"
$env:RUSTC = "$pt\rustc.exe"
& "$pt\cargo.exe" build --release --target sbf-solana-solana --workspace
$env:MAINNET_RPC_URL = "https://your-mainnet-rpc"
node scripts\kamino-mainnet-fork-roundtrip.mjs
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
