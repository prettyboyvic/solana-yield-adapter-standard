#!/usr/bin/env node
//
// MarginFi USDC live mainnet-fork roundtrip runner (Step 9).
//
// Mirrors the proven Kamino runner (scripts/kamino-mainnet-fork-roundtrip.mjs):
// spins up a local solana-test-validator that forks finalized mainnet, clones the
// committed MarginFi program + USDC bank/group/liquidity-vault/oracle, loads the
// locally built reference adapter + dispatcher SBF artifacts, then drives the real
// adapter CPI sequence end to end against the fork:
//
//   initialize_adapter -> marginfi_init -> marginfi_deposit
//                      -> current_value_cpi -> marginfi_withdraw
//
// Account orders, instruction-data encodings and PDA seeds below are taken verbatim
// from programs/reference_yield_adapter/src/lib.rs (MarginFi paths) and the committed
// account map packages/sdk/fixtures/marginfi-mainnet-fork-account-map.json.
//
// IMPORTANT: this script does NOT modify the program, does NOT touch Kamino or any
// other adapter, and does NOT flip CPI_IMPLEMENTED. It only orchestrates a fork.
//
// It MUST be run on a host that has solana-test-validator (Agave) on PATH, the SBF
// artifacts built (anchor build / cargo build-sbf), and outbound access to a mainnet
// RPC. See the bottom of this file for the exact Windows (PowerShell) command.
//
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import {
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  AccountLayout,
  TOKEN_PROGRAM_ID,
  getAccount,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

const require = createRequire(import.meta.url);
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const accountMap = JSON.parse(
  fs.readFileSync(
    path.join(repoRoot, "packages/sdk/fixtures/marginfi-mainnet-fork-account-map.json"),
    "utf8",
  ),
);

const RPC_URL = process.env.MAINNET_RPC_URL ?? "https://api.mainnet-beta.solana.com";
const LOCAL_URL = process.env.LOCAL_RPC_URL ?? "http://127.0.0.1:8899";
const DEPOSIT_AMOUNT = BigInt(process.env.MARGINFI_DEPOSIT_LAMPORTS ?? "1000000"); // 1 USDC
const COMPUTE_UNITS = Number(process.env.MARGINFI_COMPUTE_UNITS ?? "1400000");
const LEDGER = path.resolve(
  repoRoot,
  process.env.MARGINFI_FORK_LEDGER ?? "target/marginfi-mainnet-fork-ledger",
);
const FIXTURE_DIR = path.resolve(repoRoot, "target/marginfi-mainnet-fork-fixtures");
const LOG_PATH = path.join(FIXTURE_DIR, "validator.log");

// --- Program ids + committed mainnet accounts (account map === lib.rs consts) ---
const MARGINFI_PROGRAM_ID = pk(accountMap.programs.marginfi);
const REFERENCE_ADAPTER_PROGRAM_ID = pk(accountMap.programs.referenceAdapter);
const DISPATCHER_PROGRAM_ID = pk(accountMap.programs.dispatcher);
const USDC_MINT = pk(accountMap.cloneAccounts.usdcMint);
const MARGINFI_GROUP = pk(accountMap.cloneAccounts.marginfiGroup);
const USDC_BANK = pk(accountMap.cloneAccounts.usdcBank);
const BANK_LIQUIDITY_VAULT = pk(accountMap.cloneAccounts.bankLiquidityVault);
const BANK_ORACLE = pk(accountMap.cloneAccounts.bankOracle);
const BANK_LIQUIDITY_VAULT_AUTHORITY = pk(
  accountMap.derivedAccounts.bankLiquidityVaultAuthority,
);

// MARGINFI_USDC_ADAPTER_ID, verbatim from lib.rs (fixed 32-byte const, NOT a hash).
const MARGINFI_ADAPTER_ID = Buffer.from([
  0x25, 0x8d, 0x1c, 0x90, 0x9d, 0x86, 0x0b, 0x4a, 0x66, 0x11, 0x4a, 0x7f, 0x2c, 0x16, 0x7e, 0xc3,
  0x24, 0xce, 0xe4, 0x09, 0x05, 0xea, 0x3e, 0xad, 0xa1, 0x89, 0x02, 0xb9, 0xdd, 0x58, 0x3d, 0x58,
]);

const PROTOCOL_MARGINFI_USDC = 2; // ProtocolKind::MarginfiUsdc

const [STATE_PDA, STATE_BUMP] = PublicKey.findProgramAddressSync(
  [Buffer.from("adapter"), MARGINFI_ADAPTER_ID],
  REFERENCE_ADAPTER_PROGRAM_ID,
);
const user = Keypair.generate();
// Fresh MarginFi account keypair; signs marginfi_init and is reused (non-signer) thereafter.
const marginfiAccount = Keypair.generate();
const [positionPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("position"), MARGINFI_ADAPTER_ID, user.publicKey.toBuffer()],
  REFERENCE_ADAPTER_PROGRAM_ID,
);
const userUsdc = getAssociatedTokenAddressSync(USDC_MINT, user.publicKey);
const adapterVault = getAssociatedTokenAddressSync(USDC_MINT, STATE_PDA, true);

// Self-check derived accounts against the committed account map before doing anything.
assertEqual("derived adapterState", STATE_PDA.toBase58(), accountMap.derivedAccounts.adapterState);
assertEqual("derived adapterVault", adapterVault.toBase58(), accountMap.derivedAccounts.adapterVault);

const cloneAccounts = [USDC_MINT, MARGINFI_GROUP, USDC_BANK, BANK_LIQUIDITY_VAULT, BANK_ORACLE];

const started = Date.now();
let validator;

try {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  writeSystemAccountFixture(path.join(FIXTURE_DIR, "user-sol.json"), user.publicKey, 100n * 1_000_000_000n);
  writeTokenAccountFixture(path.join(FIXTURE_DIR, "user-usdc.json"), userUsdc, user.publicKey, DEPOSIT_AMOUNT * 2n);
  writeTokenAccountFixture(path.join(FIXTURE_DIR, "adapter-vault-usdc.json"), adapterVault, STATE_PDA, 0n);

  const mainnet = new Connection(RPC_URL, "confirmed");
  const forkSlot = await mainnet.getSlot("finalized");
  const validatorArgs = validatorArgsFor(forkSlot);

  console.log("== MarginFi mainnet-fork roundtrip ==");
  console.log(`rpc=${RPC_URL}`);
  console.log(`forkSlot=${forkSlot}`);
  console.log(`evidenceSlot=${accountMap.evidenceSlot}`);
  console.log(`ledger=${LEDGER}`);
  console.log(`user=${user.publicKey.toBase58()}`);
  console.log(`state=${STATE_PDA.toBase58()} bump=${STATE_BUMP}`);
  console.log(`position=${positionPda.toBase58()}`);
  console.log(`userUsdc=${userUsdc.toBase58()}`);
  console.log(`adapterVault=${adapterVault.toBase58()}`);
  console.log(`marginfiAccount=${marginfiAccount.publicKey.toBase58()}`);
  console.log(`depositAmount=${DEPOSIT_AMOUNT}`);
  console.log("\nPowerShell validator command:");
  console.log(formatPowerShellCommand(validatorArgs));

  const validatorBin = process.platform === "win32" ? "solana-test-validator.exe" : "solana-test-validator";
  validator = spawn(validatorBin, validatorArgs, { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
  const logStream = fs.createWriteStream(LOG_PATH, { flags: "w" });
  validator.stdout.pipe(logStream);
  validator.stderr.pipe(logStream);
  validator.on("exit", (code, signal) => {
    logStream.write(`\nvalidator exit code=${code} signal=${signal}\n`);
    logStream.end();
  });

  const connection = new Connection(LOCAL_URL, "confirmed");
  await waitForValidator(connection, validator);

  const evidence = {
    forkSlot,
    evidenceSlot: accountMap.evidenceSlot,
    localRpc: LOCAL_URL,
    user: user.publicKey.toBase58(),
    state: STATE_PDA.toBase58(),
    position: positionPda.toBase58(),
    userUsdc: userUsdc.toBase58(),
    adapterVault: adapterVault.toBase58(),
    marginfiAccount: marginfiAccount.publicKey.toBase58(),
    depositAmount: DEPOSIT_AMOUNT.toString(),
    signatures: {},
    computeUnits: {},
    balances: {},
    stateFields: {},
    positionFields: {},
  };

  evidence.balances.before = await balances(connection);
  evidence.stateFields.before = await adapterState(connection);
  evidence.positionFields.before = await positionState(connection);

  await sendLabeled(connection, evidence, "initializeAdapter", [ixInitializeAdapter()], []);
  await sendLabeled(connection, evidence, "marginfiInit", [ixMarginfiInit()], [marginfiAccount]);

  evidence.balances.afterInit = await balances(connection);
  evidence.stateFields.afterInit = await adapterState(connection);

  await sendLabeled(connection, evidence, "deposit", [ixMarginfiDeposit()], []);
  evidence.balances.afterDeposit = await balances(connection);
  evidence.stateFields.afterDeposit = await adapterState(connection);
  evidence.positionFields.afterDeposit = await positionState(connection);

  // Validation: deposit reduced the user's USDC source.
  const userBefore = BigInt(evidence.balances.before.userUsdc ?? "0");
  const userAfterDeposit = BigInt(evidence.balances.afterDeposit.userUsdc ?? "0");
  if (userAfterDeposit !== userBefore - DEPOSIT_AMOUNT) {
    throw new Error(
      `deposit did not reduce user USDC by ${DEPOSIT_AMOUNT}: before=${userBefore} after=${userAfterDeposit}`,
    );
  }

  await sendLabeled(connection, evidence, "currentValue", [ixCurrentValueCpi()], []);
  evidence.stateFields.afterCurrentValue = await adapterState(connection);
  evidence.positionFields.afterCurrentValue = await positionState(connection);

  const valueAssets = BigInt(evidence.stateFields.afterCurrentValue?.totalAssets ?? "0");
  if (valueAssets <= 0n) {
    throw new Error(`current_value returned non-positive total_assets: ${valueAssets}`);
  }

  const shares = BigInt(evidence.positionFields.afterCurrentValue?.shares ?? "0");
  if (shares <= 0n) throw new Error(`expected shares after current_value, got ${shares}`);

  await sendLabeled(connection, evidence, "withdraw", [ixMarginfiWithdraw(shares)], []);
  evidence.balances.afterWithdraw = await balances(connection);
  evidence.stateFields.afterWithdraw = await adapterState(connection);
  evidence.positionFields.afterWithdraw = await positionState(connection);

  // Validation: withdraw returned assets to the user destination path.
  const userAfterWithdraw = BigInt(evidence.balances.afterWithdraw.userUsdc ?? "0");
  const redeemed = userAfterWithdraw - userAfterDeposit;
  if (redeemed <= 0n) {
    throw new Error(`withdraw returned no assets to user: delta=${redeemed}`);
  }
  evidence.redeemedToUser = redeemed.toString();
  evidence.finalUserDeltaVsStart = (userAfterWithdraw - userBefore).toString();
  evidence.elapsedMs = Date.now() - started;

  console.log("\n== Evidence ==");
  console.log(JSON.stringify(evidence, null, 2));
  console.log(`\nvalidatorLog=${LOG_PATH}`);
  console.log("\nROUNDTRIP_OK");
} catch (error) {
  console.error("\n== Roundtrip failed ==");
  console.error(error?.stack ?? error);
  console.error(`validatorLog=${LOG_PATH}`);
  process.exitCode = 1;
} finally {
  if (validator) validator.kill();
}

// ---------------------------------------------------------------------------
// Validator args
// ---------------------------------------------------------------------------
function validatorArgsFor(forkSlot) {
  const referenceSo = path.join(repoRoot, "target/sbf-solana-solana/release/reference_yield_adapter.so");
  const dispatcherSo = path.join(repoRoot, "target/sbf-solana-solana/release/yield_adapter_dispatcher.so");
  for (const file of [referenceSo, dispatcherSo]) {
    if (!fs.existsSync(file)) throw new Error(`missing SBF artifact: ${file}`);
  }
  return [
    "--url",
    RPC_URL,
    "--ledger",
    LEDGER,
    "--reset",
    "--warp-slot",
    String(forkSlot),
    "--compute-unit-limit",
    String(COMPUTE_UNITS),
    "--transaction-account-lock-limit",
    "128",
    "--clone-upgradeable-program",
    MARGINFI_PROGRAM_ID.toBase58(),
    "--bpf-program",
    REFERENCE_ADAPTER_PROGRAM_ID.toBase58(),
    referenceSo,
    "--bpf-program",
    DISPATCHER_PROGRAM_ID.toBase58(),
    dispatcherSo,
    ...cloneAccounts.flatMap((account) => ["--clone", account.toBase58()]),
    "--account",
    user.publicKey.toBase58(),
    path.join(FIXTURE_DIR, "user-sol.json"),
    "--account",
    userUsdc.toBase58(),
    path.join(FIXTURE_DIR, "user-usdc.json"),
    "--account",
    adapterVault.toBase58(),
    path.join(FIXTURE_DIR, "adapter-vault-usdc.json"),
  ];
}

// ---------------------------------------------------------------------------
// Instructions (account orders + data verbatim from lib.rs MarginFi paths)
// ---------------------------------------------------------------------------
function ixInitializeAdapter() {
  // sighash + adapter_id + Borsh(AdapterConfigInput{ protocol, underlying_mint,
  //   receipt_mint, protocol_market, value_oracle, metadata_uri }).
  const metadata = Buffer.from("ipfs://solana-yield-adapters/marginfi-usdc.json", "utf8");
  const data = Buffer.alloc(8 + 32 + 1 + 32 * 4 + 4 + metadata.length);
  let o = 0;
  sighash("initialize_adapter").copy(data, o); o += 8;
  MARGINFI_ADAPTER_ID.copy(data, o); o += 32;
  data.writeUInt8(PROTOCOL_MARGINFI_USDC, o); o += 1;
  USDC_MINT.toBuffer().copy(data, o); o += 32;          // underlying_mint
  USDC_MINT.toBuffer().copy(data, o); o += 32;          // receipt_mint (unused by marginfi guards)
  MARGINFI_GROUP.toBuffer().copy(data, o); o += 32;     // protocol_market === MARGINFI_PRODUCTION_GROUP
  BANK_ORACLE.toBuffer().copy(data, o); o += 32;        // value_oracle === bank oracle
  data.writeUInt32LE(metadata.length, o); o += 4;
  metadata.copy(data, o);
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [
      { pubkey: STATE_PDA, isSigner: false, isWritable: true },
      { pubkey: user.publicKey, isSigner: true, isWritable: true },  // payer
      { pubkey: user.publicKey, isSigner: true, isWritable: false }, // authority
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

function ixMarginfiInit() {
  // MarginfiInit accounts: user, state(PDA, ro), marginfi_program, marginfi_group,
  // marginfi_account(signer, mut), system_program.
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [
      { pubkey: user.publicKey, isSigner: true, isWritable: true },
      { pubkey: STATE_PDA, isSigner: false, isWritable: false },
      { pubkey: MARGINFI_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: MARGINFI_GROUP, isSigner: false, isWritable: false },
      { pubkey: marginfiAccount.publicKey, isSigner: true, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([sighash("marginfi_init"), MARGINFI_ADAPTER_ID]),
  });
}

function ixMarginfiDeposit() {
  const data = Buffer.alloc(8 + 32 + 8 + 8);
  sighash("marginfi_deposit").copy(data, 0);
  MARGINFI_ADAPTER_ID.copy(data, 8);
  data.writeBigUInt64LE(DEPOSIT_AMOUNT, 40); // amount
  data.writeBigUInt64LE(0n, 48);             // min_shares_out
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [...adapterCpiFixedKeys(), ...marginfiDepositRemainingKeys()],
    data,
  });
}

function ixCurrentValueCpi() {
  // remaining_accounts = [usdc_bank (ro), marginfi_account (ro)]
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [
      ...adapterCpiFixedKeys(),
      { pubkey: USDC_BANK, isSigner: false, isWritable: false },
      { pubkey: marginfiAccount.publicKey, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([sighash("current_value_cpi"), MARGINFI_ADAPTER_ID]),
  });
}

function ixMarginfiWithdraw(shares) {
  const data = Buffer.alloc(8 + 32 + 8 + 8);
  sighash("marginfi_withdraw").copy(data, 0);
  MARGINFI_ADAPTER_ID.copy(data, 8);
  data.writeBigUInt64LE(shares, 40);  // shares
  data.writeBigUInt64LE(0n, 48);      // min_assets_out
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [...adapterCpiFixedKeys(), ...marginfiWithdrawRemainingKeys()],
    data,
  });
}

// AdapterCpiRoute fixed prefix (shared with Kamino): user, state, position,
// user_underlying, adapter_underlying, token_program, system_program.
function adapterCpiFixedKeys() {
  return [
    { pubkey: user.publicKey, isSigner: true, isWritable: true },
    { pubkey: STATE_PDA, isSigner: false, isWritable: true },
    { pubkey: positionPda, isSigner: false, isWritable: true },
    { pubkey: userUsdc, isSigner: false, isWritable: true },
    { pubkey: adapterVault, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];
}

// 7 lending_account_deposit metas (outer specs: state authority writable-promoted,
// inner meta stays readonly inside the program) + executable MarginFi program.
function marginfiDepositRemainingKeys() {
  return [
    { pubkey: MARGINFI_GROUP, isSigner: false, isWritable: false },
    { pubkey: marginfiAccount.publicKey, isSigner: false, isWritable: true },
    { pubkey: STATE_PDA, isSigner: false, isWritable: true }, // authority (state PDA), outer-promoted
    { pubkey: USDC_BANK, isSigner: false, isWritable: true },
    { pubkey: adapterVault, isSigner: false, isWritable: true }, // signer_token_account
    { pubkey: BANK_LIQUIDITY_VAULT, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: MARGINFI_PROGRAM_ID, isSigner: false, isWritable: false }, // executable, for invoke_signed
  ];
}

// 8 lending_account_withdraw base metas + 2 health metas [bank, oracle] +
// executable MarginFi program. Outer-promoted writable at slot 2 (state) and 8 (health bank).
function marginfiWithdrawRemainingKeys() {
  return [
    { pubkey: MARGINFI_GROUP, isSigner: false, isWritable: true },
    { pubkey: marginfiAccount.publicKey, isSigner: false, isWritable: true },
    { pubkey: STATE_PDA, isSigner: false, isWritable: true }, // authority (state PDA), outer-promoted
    { pubkey: USDC_BANK, isSigner: false, isWritable: true },
    { pubkey: adapterVault, isSigner: false, isWritable: true }, // destination_token_account
    { pubkey: BANK_LIQUIDITY_VAULT_AUTHORITY, isSigner: false, isWritable: false },
    { pubkey: BANK_LIQUIDITY_VAULT, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: USDC_BANK, isSigner: false, isWritable: true }, // health bank, outer-promoted
    { pubkey: BANK_ORACLE, isSigner: false, isWritable: false }, // health oracle
    { pubkey: MARGINFI_PROGRAM_ID, isSigner: false, isWritable: false }, // executable, for invoke_signed
  ];
}

// ---------------------------------------------------------------------------
// Fixtures + helpers (shared shape with the Kamino runner)
// ---------------------------------------------------------------------------
function writeSystemAccountFixture(file, pubkey, lamports) {
  fs.writeFileSync(
    file,
    `${JSON.stringify(
      {
        pubkey: pubkey.toBase58(),
        account: {
          lamports: Number(lamports),
          data: ["", "base64"],
          owner: SystemProgram.programId.toBase58(),
          executable: false,
          rentEpoch: 0,
          space: 0,
        },
      },
      null,
      2,
    )}\n`,
  );
}

function writeTokenAccountFixture(file, pubkey, owner, amount) {
  const data = Buffer.alloc(AccountLayout.span);
  AccountLayout.encode(
    {
      mint: USDC_MINT,
      owner,
      amount,
      delegateOption: 0,
      delegate: PublicKey.default,
      state: 1,
      isNativeOption: 0,
      isNative: 0n,
      delegatedAmount: 0n,
      closeAuthorityOption: 0,
      closeAuthority: PublicKey.default,
    },
    data,
  );
  fs.writeFileSync(
    file,
    `${JSON.stringify(
      {
        pubkey: pubkey.toBase58(),
        account: {
          lamports: 2039280,
          data: [data.toString("base64"), "base64"],
          owner: TOKEN_PROGRAM_ID.toBase58(),
          executable: false,
          rentEpoch: 0,
          space: AccountLayout.span,
        },
      },
      null,
      2,
    )}\n`,
  );
}

function formatPowerShellCommand(args) {
  return ["solana-test-validator", ...args]
    .map((part) => (/[\s;]/.test(part) ? `"${part.replaceAll('"', '`"')}"` : part))
    .join(" ");
}

async function waitForValidator(connection, child) {
  const startedAt = Date.now();
  let exit;
  child.once("exit", (code, signal) => {
    exit = { code, signal };
  });
  while (Date.now() - startedAt < 120_000) {
    if (exit) throw new Error(`validator exited before RPC was ready: ${JSON.stringify(exit)}`);
    try {
      await connection.getVersion();
      await connection.getLatestBlockhash("processed");
      await connection.getSlot("processed");
      return;
    } catch {
      await sleep(1000);
    }
  }
  throw new Error("validator RPC did not become ready within 120s");
}

async function sendLabeled(connection, evidence, label, instructions, extraSigners) {
  console.log(`${label}: sending ${instructions.length} instruction(s)`);
  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: COMPUTE_UNITS }),
    ...instructions,
  );
  try {
    const sig = await withTimeout(
      sendAndConfirmTransaction(connection, tx, [user, ...extraSigners], {
        commitment: "confirmed",
        skipPreflight: false,
      }),
      90_000,
      `${label} confirmation timed out`,
    );
    evidence.signatures[label] = sig;
    const details = await connection.getTransaction(sig, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    evidence.computeUnits[label] = details?.meta?.computeUnitsConsumed ?? null;
    evidence[`${label}Logs`] = details?.meta?.logMessages ?? [];
    console.log(`${label}: ${sig} CU=${evidence.computeUnits[label]}`);
  } catch (error) {
    if (typeof error?.getLogs === "function") {
      try {
        console.error((await error.getLogs(connection)).join("\n"));
      } catch {
        // ignore log fetch errors
      }
    }
    throw error;
  }
}

function withTimeout(promise, ms, message) {
  let timeout;
  return Promise.race([
    promise.finally(() => clearTimeout(timeout)),
    new Promise((_, reject) => {
      timeout = setTimeout(() => reject(new Error(message)), ms);
    }),
  ]);
}

async function balances(connection) {
  return {
    userUsdc: await tokenAmount(connection, userUsdc),
    adapterVault: await tokenAmount(connection, adapterVault),
    bankLiquidityVault: await tokenAmount(connection, BANK_LIQUIDITY_VAULT),
  };
}

async function tokenAmount(connection, address) {
  try {
    const account = await getAccount(connection, address);
    return account.amount.toString();
  } catch {
    return null;
  }
}

async function adapterState(connection) {
  const info = await connection.getAccountInfo(STATE_PDA);
  if (!info) return null;
  const data = info.data;
  return {
    totalAssets: data.readBigUInt64LE(201).toString(),
    totalShares: data.readBigUInt64LE(209).toString(),
    lastUpdateSlot: data.readBigUInt64LE(217).toString(),
    bump: data.readUInt8(225),
    paused: Boolean(data.readUInt8(226)),
  };
}

async function positionState(connection) {
  const info = await connection.getAccountInfo(positionPda);
  if (!info) return null;
  const data = info.data;
  return {
    shares: data.readBigUInt64LE(72).toString(),
    principalAssets: data.readBigUInt64LE(80).toString(),
    lastValueAssets: data.readBigUInt64LE(88).toString(),
    bump: data.readUInt8(96),
  };
}

function sighash(name) {
  return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

function assertEqual(label, actual, expectedValue) {
  if (actual !== expectedValue) {
    throw new Error(`${label} mismatch: actual=${actual} expected=${expectedValue}`);
  }
}

function pk(value) {
  return new PublicKey(value);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------
// Exact Windows (PowerShell) command, run from the repo root:
//
//   $env:MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com"   # or a paid RPC
//   node scripts/marginfi-mainnet-fork-roundtrip.mjs
//
// Prerequisites on the host:
//   * solana-test-validator.exe (Agave) on PATH
//   * SBF artifacts built: anchor build   (produces
//     target/sbf-solana-solana/release/{reference_yield_adapter,yield_adapter_dispatcher}.so)
//   * outbound network access to the mainnet RPC
//
// A successful run prints "ROUNDTRIP_OK" plus a JSON evidence block (fork slot,
// signatures, compute units, balance deltas, current_value total_assets).
// ---------------------------------------------------------------------------
