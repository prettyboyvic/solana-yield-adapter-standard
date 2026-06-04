/**
 * Regenerate packages/sdk/fixtures/marginfi-cpi-account-plan.json from the
 * committed docs/marginfi-derived-accounts.json.
 *
 * Offline + deterministic. The instruction account ORDER, isSigner/isWritable
 * flags, and discriminators are the IDL-proven canonical values
 * (@mrgnlabs/marginfi-client-v2 6.4.2, IDL marginfi 0.1.7). Concrete pubkeys come
 * from the derived map; the oracle and the runtime marginfiAccount/feePayer are
 * marked generatedAtRuntime. This emits an account plan only; it does NOT imply
 * MarginFi CPI is implemented.
 *
 * Run: npm run marginfi:cpi-plan
 */
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { PublicKey } from "@solana/web3.js";

const RUNTIME = "GENERATED_AT_RUNTIME";
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const derivedPath = path.resolve(__dirname, "../docs/marginfi-derived-accounts.json");
const fixturePath = path.resolve(
  __dirname,
  "../packages/sdk/fixtures/marginfi-cpi-account-plan.json",
);

const d = JSON.parse(fs.readFileSync(derivedPath, "utf8"));

// Integrity: re-derive the bank liquidity-vault PDAs and assert they match the
// committed derived map (seeds from marginfi-client-v2 constants.ts).
const prog = new PublicKey(d.program);
const bank = new PublicKey(d.usdcBank);
const lv = PublicKey.findProgramAddressSync(
  [Buffer.from("liquidity_vault"), bank.toBuffer()],
  prog,
)[0].toBase58();
const lva = PublicKey.findProgramAddressSync(
  [Buffer.from("liquidity_vault_auth"), bank.toBuffer()],
  prog,
)[0].toBase58();
if (lv !== d.liquidityVault || lva !== d.liquidityVaultAuthority) {
  throw new Error("derived liquidity-vault PDAs do not match docs/marginfi-derived-accounts.json");
}

const acc = (
  name: string,
  pubkey: string,
  isSigner: boolean,
  isWritable: boolean,
  generatedAtRuntime = false,
) => ({ name, pubkey, isSigner, isWritable, generatedAtRuntime });

const fixture = {
  schema: "marginfi-cpi-account-plan/v1",
  source: "docs/marginfi-derived-accounts.json + @mrgnlabs/marginfi-client-v2@6.4.2 IDL (marginfi 0.1.7)",
  generatedBy: "scripts/export-marginfi-cpi-plan.ts",
  rustBridge:
    "Account order + isSigner/isWritable + instruction discriminators are canonical (IDL-proven). Concrete pubkeys are resolved offline; oracle/oracleSetup/oracleMaxAge and the runtime marginfiAccount/feePayer are explicitly marked generatedAtRuntime. This fixture does NOT imply MarginFi CPI is implemented.",
  program: d.program,
  group: d.group,
  underlyingMint: d.underlyingMint,
  usdcBank: d.usdcBank,
  bankSelection: {
    selectedBy: "mint",
    rule: "Bank in the marginfi production group whose on-chain Bank.mint == underlyingMint (USDC). Confirmed via the official marginfi bank-metadata cache (single tokenSymbol=USDC entry whose tokenAddress == USDC mint) and MUST be re-verified on-chain (Bank.group == group AND Bank.mint == underlyingMint) by scripts/derive-marginfi-accounts.ts.",
    candidates: [d.usdcBank],
  },
  oracle: {
    key: RUNTIME,
    setup: RUNTIME,
    maxAge: RUNTIME,
    note: "Bank.config.oracle_keys[0] / oracle_setup / oracle_max_age, read from the on-chain USDC bank. Required only for lending_account_withdraw health checks; current_value is oracle-independent.",
  },
  custody: {
    marginfiAccountAuthority: d.adapterAuthorityStatePda,
    adapterUnderlyingVault: d.adapterUnderlyingVault,
    note: 'authority = adapter state PDA ([b"adapter", adapter_id]) for adapter_id = sha256("solana-yield-adapter:marginfi-usdc"); adapterUnderlyingVault = the state-PDA USDC ATA used as signer/destination token account. marginfiAccount is a fresh runtime keypair whose authority is the state PDA.',
  },
  txShape:
    "Split-transaction shape not final. Step 2 only emits canonical account plans; no MarginFi Rust entrypoints (init/deposit/withdraw) and no current_value wiring exist yet. CPI_IMPLEMENTED remains false.",
  plans: {
    marginfi_account_initialize: {
      instruction: "marginfi_account_initialize",
      discriminator: d.instructionDiscriminators.marginfi_account_initialize,
      argNames: [],
      accounts: [
        acc("marginfi_group", d.group, false, false),
        acc("marginfi_account", RUNTIME, true, true, true),
        acc("authority", d.adapterAuthorityStatePda, true, false),
        acc("fee_payer", RUNTIME, true, true, true),
        acc("system_program", d.systemProgram, false, false),
      ],
    },
    lending_account_deposit: {
      instruction: "lending_account_deposit",
      discriminator: d.instructionDiscriminators.lending_account_deposit,
      argNames: ["amount", "deposit_up_to_limit"],
      accounts: [
        acc("group", d.group, false, false),
        acc("marginfi_account", RUNTIME, false, true, true),
        acc("authority", d.adapterAuthorityStatePda, true, false),
        acc("bank", d.usdcBank, false, true),
        acc("signer_token_account", d.adapterUnderlyingVault, false, true),
        acc("liquidity_vault", d.liquidityVault, false, true),
        acc("token_program", d.tokenProgram, false, false),
      ],
    },
    lending_account_withdraw: {
      instruction: "lending_account_withdraw",
      discriminator: d.instructionDiscriminators.lending_account_withdraw,
      argNames: ["amount", "withdraw_all"],
      accounts: [
        acc("group", d.group, false, true),
        acc("marginfi_account", RUNTIME, false, true, true),
        acc("authority", d.adapterAuthorityStatePda, true, false),
        acc("bank", d.usdcBank, false, true),
        acc("destination_token_account", d.adapterUnderlyingVault, false, true),
        acc("bank_liquidity_vault_authority", d.liquidityVaultAuthority, false, false),
        acc("liquidity_vault", d.liquidityVault, false, true),
        acc("token_program", d.tokenProgram, false, false),
      ],
      healthRemainingAccounts: [
        acc("bank", d.usdcBank, false, false),
        acc("oracle", RUNTIME, false, false, true),
      ],
    },
  },
};

fs.writeFileSync(fixturePath, JSON.stringify(fixture, null, 2) + "\n");
console.log("wrote", path.relative(process.cwd(), fixturePath));
