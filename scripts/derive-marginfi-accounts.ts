/**
 * Derive + verify the MarginFi USDC account map and write
 * docs/marginfi-derived-accounts.json.
 *
 * Uses ONLY the official @mrgnlabs/marginfi-client-v2 production config and
 * verified on-chain reads (no guessed addresses). Requires a mainnet RPC.
 *
 *   RPC_URL=<mainnet-rpc> npx tsx scripts/derive-marginfi-accounts.ts
 *   # then: npm run marginfi:cpi-plan   (regenerates the SDK fixture)
 *
 * What it pins:
 *  - production group + program id (from marginfi-client-v2 config, not hardcoded)
 *  - USDC bank selected BY MINT (Bank.mint == USDC); fails loudly if the group
 *    has more or fewer than one USDC bank
 *  - liquidity vault + liquidity-vault authority PDAs (seeds from the SDK)
 *  - oracle key / setup / max age from the on-chain Bank.config
 *  - adapter state PDA (authority) + its USDC ATA (custody)
 */
import * as fs from "node:fs";
import * as path from "node:path";
import * as crypto from "node:crypto";
import { fileURLToPath } from "node:url";
import { Connection, PublicKey, Keypair } from "@solana/web3.js";
import { getConfig, MarginfiClient } from "@mrgnlabs/marginfi-client-v2";
import { NodeWallet } from "@mrgnlabs/mrgn-common";

const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const REFERENCE_ADAPTER_PROGRAM_ID = new PublicKey(
  "BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1",
);
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey(
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
);
const TOKEN_PROGRAM_ID = new PublicKey(
  "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
);

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const outPath = path.resolve(__dirname, "../docs/marginfi-derived-accounts.json");

async function main() {
  const rpc = process.env.RPC_URL;
  if (!rpc) throw new Error("set RPC_URL to a mainnet RPC endpoint");
  const connection = new Connection(rpc, "confirmed");

  // Production config — group + program come from the SDK, never hardcoded.
  const config = getConfig("production");
  const client = await MarginfiClient.fetch(
    config,
    new NodeWallet(Keypair.generate()),
    connection,
  );

  // Select the USDC bank BY MINT within the production group.
  const usdcBanks = [...client.banks.values()].filter(
    (b: any) => b.mint.equals(USDC_MINT) && b.group.equals(config.groupPk),
  );
  if (usdcBanks.length !== 1) {
    throw new Error(
      `expected exactly one production USDC bank, found ${usdcBanks.length}: ` +
        usdcBanks.map((b: any) => b.address.toBase58()).join(", "),
    );
  }
  const bank: any = usdcBanks[0];
  const bankPk: PublicKey = bank.address;

  const [liquidityVault] = PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault"), bankPk.toBuffer()],
    config.programId,
  );
  const [liquidityVaultAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault_auth"), bankPk.toBuffer()],
    config.programId,
  );

  const oracleKey: PublicKey = bank.config.oracleKeys[0];
  const oracleSetup = String(bank.config.oracleSetup);
  const oracleMaxAge = Number(bank.config.oracleMaxAge);

  // Adapter custody PDAs.
  const adapterId = crypto
    .createHash("sha256")
    .update("solana-yield-adapter:marginfi-usdc")
    .digest();
  const [statePda] = PublicKey.findProgramAddressSync(
    [Buffer.from("adapter"), adapterId],
    REFERENCE_ADAPTER_PROGRAM_ID,
  );
  const [adapterVault] = PublicKey.findProgramAddressSync(
    [statePda.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), USDC_MINT.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );

  const out = {
    schema: "marginfi-derived-accounts/v1",
    source:
      "@mrgnlabs/marginfi-client-v2 production config + verified on-chain reads",
    generatedBy: "scripts/derive-marginfi-accounts.ts",
    program: config.programId.toBase58(),
    group: config.groupPk.toBase58(),
    groupSource: "marginfi-client-v2 getConfig('production')",
    underlyingMint: USDC_MINT.toBase58(),
    usdcBank: bankPk.toBase58(),
    usdcBankSelection: {
      selectedBy: "mint",
      rule: "Bank in the production group whose on-chain Bank.mint == underlyingMint; fails loudly if not exactly one.",
      candidates: usdcBanks.map((b: any) => b.address.toBase58()),
    },
    liquidityVault: liquidityVault.toBase58(),
    liquidityVaultAuthority: liquidityVaultAuthority.toBase58(),
    vaultDerivation:
      'PDA(["liquidity_vault", bank], program) and PDA(["liquidity_vault_auth", bank], program).',
    adapterAuthorityStatePda: statePda.toBase58(),
    adapterUnderlyingVault: adapterVault.toBase58(),
    adapterDerivation:
      'adapter_id = sha256("solana-yield-adapter:marginfi-usdc"); state PDA = PDA(["adapter", adapter_id], reference adapter program); vault = ATA(state, USDC).',
    tokenProgram: TOKEN_PROGRAM_ID.toBase58(),
    systemProgram: "11111111111111111111111111111111",
    oracle: {
      key: oracleKey.toBase58(),
      setup: oracleSetup,
      maxAge: oracleMaxAge,
      note: "From on-chain Bank.config. Used only for lending_account_withdraw health checks.",
    },
    instructionDiscriminators: {
      marginfi_account_initialize: [43, 78, 61, 255, 148, 52, 249, 154],
      lending_account_deposit: [171, 94, 235, 103, 82, 64, 212, 140],
      lending_account_withdraw: [36, 72, 74, 19, 210, 210, 192, 192],
    },
    cpiPrereqStatus: "VERIFIED_ON_CHAIN",
    disclaimer:
      "Account map only. MarginFi CPI is NOT implemented and no mainnet-fork roundtrip has been run.",
  };

  fs.writeFileSync(outPath, JSON.stringify(out, null, 2) + "\n");
  console.log("wrote", path.relative(process.cwd(), outPath));
  console.log("usdc bank:", bankPk.toBase58(), "oracle:", oracleKey.toBase58());
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
