# Maple Syrup Gate 1 Feasibility

**Verdict: REQUIRES DESIGN CHANGE.**

Maple cannot be implemented as a native synchronous CPI adapter under the
current adapter ABI. syrupUSDC on Solana is an SPL token moved, minted, and
burned through Chainlink CCIP/CCT infrastructure. Maple's native mint and redeem
flows route requests cross-chain to Ethereum rather than invoking a synchronous
Maple lending program on Solana.

No Maple Solana program with synchronous USDC deposit and syrupUSDC withdrawal,
or syrupUSDC deposit and USDC withdrawal, was found during Gate 1.

## Verified Solana Accounts

| Role | Address | Gate 1 finding |
|---|---|---|
| syrupUSDC mint | `AvZZF1YaZDziPY2RCK4oJrRVrbN3mTD9NL24hPeaZeUj` | SPL token mint |
| CCIP Router | `Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C` | Executable Chainlink CCIP router |
| CCIP pool config | `HrTBpF3LqSxXnjnYdR4htnBLyMHNZ6eNaDZGPundvHbm` | CCIP token-pool config PDA, not a Maple lending pool |
| CCIP pool program | `787uwTCd8b2ikQP6g9AapMky36PWDv9x1XpC5ZUAfDYc` | Token burn/mint pool program |
| syrupUSDC/USDC oracle | `CpNyiFt84q66665Kx64bobxZuMgZ2EecrhAJs1HikS2T` | Chainlink `SYRUPUSDC-USDC Exchange Rate` feed |
| Whirlpool candidate | `6fteKNvMdv7tYmBoJHhj1jx6rHcEwC6RdSEmVpyS613J` | Same-chain liquidity candidate, not a Maple program |

The repository's Maple mint, router, pool-config, and oracle IDs are coherent
with the on-chain account roles. The pool-config account must not be described
as a Maple lending pool.

## Native CCIP Path

Maple's documented native Solana mint/redeem flow is asynchronous:

1. A user initiates a USDC deposit or syrupUSDC redemption request.
2. Chainlink CCIP sends a cross-chain message using a destination-chain
   selector.
3. The corresponding Maple operation is processed on Ethereum.
4. The resulting token delivery completes after cross-chain settlement.

Maple documents an approximate 10-30 minute processing time for native
cross-chain deposits and redemptions. This is not a same-chain
USDC-to-syrupUSDC mint or syrupUSDC-to-USDC redemption CPI.

The current adapter ABI requires `deposit` to credit shares and enforce
`min_shares_out` atomically, and requires `withdraw` to return underlying assets
and enforce `min_assets_out` atomically. An asynchronous CCIP message cannot
satisfy those guarantees in the initiating Solana transaction.

## Mainnet-Fork Consequence

A native Maple/CCIP roundtrip is not runnable as a synchronous Solana
mainnet-fork transaction. A local fork cannot complete the external Ethereum
operation and later CCIP settlement inside the initiating transaction.

A Solana DEX roundtrip may be technically possible using existing syrupUSDC
liquidity. The Whirlpool candidate above is evidence of a possible market route,
but that design would buy and sell Maple exposure through Solana liquidity. It
would not test or implement Maple native mint/redeem and would introduce
separate pool selection, liquidity, slippage, and market-execution risks.

A receipt-token custody route is also possible only if the adapter underlying
changes from USDC to syrupUSDC. That would be a different adapter design rather
than a USDC-to-Maple mutation path.

## Safe Current Behavior

- Keep Maple mutation routes loud-fail.
- Keep `CPI_IMPLEMENTED=false`.
- Do not claim a Maple live mainnet-fork pass or full five-adapter completion.
- Require an explicit design decision before implementing either asynchronous
  settlement, receipt-token custody, or a Solana DEX market-execution route.

## Official References

- [Maple: syrupUSDC on Solana](https://docs.maple.finance/integrate/crosschain-solana-arbitrum-base-plasma/syrupusd-crosschain)
- [Maple: native mint/redeem](https://docs.maple.finance/integrate/crosschain-solana-arbitrum-base-plasma/syrupusdc-native-mint-redeem)
- [Maple: syrupUSDC addresses](https://docs.maple.finance/integrate/syrupusdc-addresses)
- [Chainlink CCIP: syrupUSDC token directory](https://docs.chain.link/ccip/directory/mainnet/token/SYRUP_USDC)
- [Chainlink CCIP Router API](https://docs.chain.link/ccip/api-reference/svm/v1.6.0/router)
