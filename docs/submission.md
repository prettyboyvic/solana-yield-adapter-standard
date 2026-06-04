# Submission Notes

Superteam Earn listing checked on 2026-06-04:

- Title: Develop Solana Yield Adapter Standard
- Sponsor: Superteam Ukraine
- Status: OPEN
- Reward: 1,000 USDC total, split 700 / 300
- Deadline: 2026-06-09T20:59:59.999Z
- Public page: https://superteam.fun/earn/listing/develop-solana-yield-adapter-standard
- Submission count shown on page: 0
- Submission requirement: public GitHub repository link
- Important eligibility warning: the page marks the listing as regional and only open for people in Ukraine.
- Agent warning: API details show `agentAccess = HUMAN_ONLY`, so final submission should be done from a human Superteam account.

Before final submission:

1. Replace protocol account placeholders in `docs/protocol-adapters.md` and `scripts/clone-mainnet-accounts.ts`.
2. Run `anchor build` with Anchor 0.31.1 and Solana 2.2.20.
3. Deploy the dispatcher registry program to devnet.
4. Run all five mainnet-fork tests and paste the logs into this document.
5. Push to a public GitHub repository.

## Local Verification

Ran on 2026-06-04:

```text
cargo check --quiet: pass
npm test: pass, 5 SDK ABI tests
npm run build: pass
npm audit: pass, 0 vulnerabilities
```

Not yet run:

```text
anchor build
anchor test
devnet deploy
mainnet-fork adapter tests
```

Reason: Anchor CLI and Solana CLI are not installed in the current Windows shell. Rust/Cargo are available from the workspace-local `.rust` directory and were used for `cargo check`.
