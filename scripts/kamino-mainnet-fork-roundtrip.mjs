#!/usr/bin/env node
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
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  AccountLayout,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAccount,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

const require = createRequire(import.meta.url);
const {
  initObligationFarmsForReserve,
  refreshObligation,
  refreshObligationFarmsForReserve,
  refreshReserve,
} = require("@kamino-finance/klend-sdk/dist/idl_codegen/instructions");
const { Obligation } = require("@kamino-finance/klend-sdk/dist/idl_codegen/accounts");

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const derived = JSON.parse(
  fs.readFileSync(path.join(repoRoot, "docs/kamino-derived-accounts.json"), "utf8"),
);

const RPC_URL = process.env.MAINNET_RPC_URL ?? "https://api.mainnet-beta.solana.com";
const LOCAL_URL = process.env.LOCAL_RPC_URL ?? "http://127.0.0.1:8899";
const DEPOSIT_AMOUNT = BigInt(process.env.KAMINO_DEPOSIT_LAMPORTS ?? "1000000");
const COMPUTE_UNITS = Number(process.env.KAMINO_COMPUTE_UNITS ?? "1400000");
const LEDGER = path.resolve(repoRoot, process.env.KAMINO_FORK_LEDGER ?? "target/kamino-mainnet-fork-ledger");
const FIXTURE_DIR = path.resolve(repoRoot, "target/kamino-mainnet-fork-fixtures");
const LOG_PATH = path.join(FIXTURE_DIR, "validator.log");

const DISPATCHER_PROGRAM_ID = new PublicKey("37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY");
const REFERENCE_ADAPTER_PROGRAM_ID = new PublicKey("BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1");
const KLEND_PROGRAM_ID = pk(derived.klendProgramId);
const FARMS_PROGRAM_ID = new PublicKey("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
const USDC_MINT = pk(derived.underlyingMint);
const KAMINO_ADAPTER_ID = adapterId("kamino-usdc");
const [STATE_PDA, STATE_BUMP] = PublicKey.findProgramAddressSync(
  [Buffer.from("adapter"), KAMINO_ADAPTER_ID],
  REFERENCE_ADAPTER_PROGRAM_ID,
);

const user = Keypair.generate();
const [positionPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("position"), KAMINO_ADAPTER_ID, user.publicKey.toBuffer()],
  REFERENCE_ADAPTER_PROGRAM_ID,
);
const userUsdc = getAssociatedTokenAddressSync(USDC_MINT, user.publicKey);
const adapterVault = getAssociatedTokenAddressSync(USDC_MINT, STATE_PDA, true);

const accounts = {
  lendingMarket: pk(derived.lendingMarket),
  lendingMarketAuthority: pk(derived.lendingMarketAuthority),
  reserve: pk(derived.usdcReserve),
  reserveLiquiditySupply: pk(derived.reserveLiquiditySupplyVault),
  reserveCollateralMint: pk(derived.reserveCollateralMint),
  reserveDestinationCollateral: pk(derived.reserveDestinationDepositCollateral),
  scopePrices: pk(derived.oracles.scopePriceFeed),
  userMetadata: pk(derived.cpiPrereqs.userMetadata),
  obligation: pk(derived.cpiPrereqs.obligation),
  reserveFarmState: pk(derived.cpiPrereqs.reserveCollateralFarmState),
  obligationFarmState: pk(derived.cpiPrereqs.obligationFarmState),
};

const cloneAccounts = [
  USDC_MINT,
  accounts.lendingMarket,
  accounts.lendingMarketAuthority,
  accounts.reserve,
  accounts.reserveLiquiditySupply,
  accounts.reserveCollateralMint,
  accounts.reserveDestinationCollateral,
  accounts.scopePrices,
  accounts.reserveFarmState,
];

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

  console.log("== Kamino mainnet-fork roundtrip ==");
  console.log(`rpc=${RPC_URL}`);
  console.log(`forkSlot=${forkSlot}`);
  console.log(`ledger=${LEDGER}`);
  console.log(`user=${user.publicKey.toBase58()}`);
  console.log(`state=${STATE_PDA.toBase58()} bump=${STATE_BUMP}`);
  console.log(`position=${positionPda.toBase58()}`);
  console.log(`userUsdc=${userUsdc.toBase58()}`);
  console.log(`adapterVault=${adapterVault.toBase58()}`);
  console.log("\nPowerShell validator command:");
  console.log(formatPowerShellCommand(validatorArgs));

  const validatorBin = process.platform === "win32" ? "solana-test-validator.exe" : "solana-test-validator";
  validator = spawn(validatorBin, validatorArgs, {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });
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
    localRpc: LOCAL_URL,
    user: user.publicKey.toBase58(),
    state: STATE_PDA.toBase58(),
    position: positionPda.toBase58(),
    userUsdc: userUsdc.toBase58(),
    adapterVault: adapterVault.toBase58(),
    obligation: accounts.obligation.toBase58(),
    obligationFarmState: accounts.obligationFarmState.toBase58(),
    signatures: {},
    computeUnits: {},
    balances: {},
    obligationCollateral: {},
    stateFields: {},
    positionFields: {},
  };

  evidence.balances.before = await balances(connection);
  evidence.obligationCollateral.before = await obligationCollateral(connection);
  evidence.stateFields.before = await adapterState(connection);
  evidence.positionFields.before = await positionState(connection);

  await sendLabeled(connection, evidence, "initializeAdapter", [
    ixInitializeAdapter(),
  ]);
  await sendLabeled(connection, evidence, "kaminoInit", [ixKaminoInit()]);
  await sendLabeled(connection, evidence, "initObligationFarm", [ixInitObligationFarm()]);

  evidence.balances.afterInit = await balances(connection);
  evidence.obligationCollateral.afterInit = await obligationCollateral(connection);
  evidence.stateFields.afterInit = await adapterState(connection);
  evidence.positionFields.afterInit = await positionState(connection);

  await sendLabeled(connection, evidence, "deposit", [
    ixRefreshReserve(),
    ixRefreshObligation(),
    ixKaminoDeposit(),
  ]);
  await sendLabeled(connection, evidence, "refreshFarmAfterDeposit", [
    ixRefreshReserve(),
    ixRefreshObligation({ includeDepositReserve: true }),
    ixRefreshObligationFarm(),
  ]);
  evidence.balances.afterDeposit = await balances(connection);
  evidence.obligationCollateral.afterDeposit = await obligationCollateral(connection);
  evidence.stateFields.afterDeposit = await adapterState(connection);
  evidence.positionFields.afterDeposit = await positionState(connection);

  await sendLabeled(connection, evidence, "currentValue", [
    ixRefreshReserve(),
    ixRefreshObligation({ includeDepositReserve: true }),
    ixCurrentValueCpi(),
  ]);
  evidence.balances.afterCurrentValue = await balances(connection);
  evidence.obligationCollateral.afterCurrentValue = await obligationCollateral(connection);
  evidence.stateFields.afterCurrentValue = await adapterState(connection);
  evidence.positionFields.afterCurrentValue = await positionState(connection);

  const shares = BigInt(evidence.positionFields.afterCurrentValue?.shares ?? "0");
  if (shares <= 0n) throw new Error(`expected shares after current_value, got ${shares}`);
  await sendLabeled(connection, evidence, "withdraw", [
    ixRefreshReserve(),
    ixRefreshObligation({ includeDepositReserve: true }),
    ixKaminoWithdraw(shares),
  ]);
  evidence.balances.afterWithdraw = await balances(connection);
  evidence.obligationCollateral.afterWithdraw = await obligationCollateral(connection);
  evidence.stateFields.afterWithdraw = await adapterState(connection);
  evidence.positionFields.afterWithdraw = await positionState(connection);
  evidence.elapsedMs = Date.now() - started;

  console.log("\n== Evidence ==");
  console.log(JSON.stringify(evidence, null, 2));
  console.log(`\nvalidatorLog=${LOG_PATH}`);
} catch (error) {
  console.error("\n== Roundtrip failed ==");
  console.error(error?.stack ?? error);
  console.error(`validatorLog=${LOG_PATH}`);
  process.exitCode = 1;
} finally {
  if (validator) {
    validator.kill();
  }
}

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
    KLEND_PROGRAM_ID.toBase58(),
    "--clone-upgradeable-program",
    FARMS_PROGRAM_ID.toBase58(),
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

function formatPowerShellCommand(args) {
  const parts = ["solana-test-validator", ...args].map((part) =>
    /[\s;]/.test(part) ? `"${part.replaceAll('"', '`"')}"` : part,
  );
  return parts.join(" ");
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

async function sendLabeled(connection, evidence, label, instructions) {
  console.log(`${label}: sending ${instructions.length} instruction(s)`);
  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: COMPUTE_UNITS }),
    ...instructions,
  );
  try {
    const sig = await withTimeout(
      sendAndConfirmTransaction(connection, tx, [user], {
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

function ixInitializeAdapter() {
  const metadata = Buffer.from("ipfs://solana-yield-adapters/kamino-usdc.json", "utf8");
  const data = Buffer.alloc(8 + 32 + 1 + 32 * 4 + 4 + metadata.length);
  let o = 0;
  sighash("initialize_adapter").copy(data, o);
  o += 8;
  KAMINO_ADAPTER_ID.copy(data, o);
  o += 32;
  data.writeUInt8(1, o);
  o += 1;
  USDC_MINT.toBuffer().copy(data, o);
  o += 32;
  accounts.reserveCollateralMint.toBuffer().copy(data, o);
  o += 32;
  accounts.lendingMarket.toBuffer().copy(data, o);
  o += 32;
  accounts.scopePrices.toBuffer().copy(data, o);
  o += 32;
  data.writeUInt32LE(metadata.length, o);
  o += 4;
  metadata.copy(data, o);
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [
      { pubkey: STATE_PDA, isSigner: false, isWritable: true },
      { pubkey: user.publicKey, isSigner: true, isWritable: true },
      { pubkey: user.publicKey, isSigner: true, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

function ixKaminoInit() {
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [
      { pubkey: user.publicKey, isSigner: true, isWritable: true },
      { pubkey: STATE_PDA, isSigner: false, isWritable: false },
      { pubkey: KLEND_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: accounts.userMetadata, isSigner: false, isWritable: true },
      { pubkey: accounts.obligation, isSigner: false, isWritable: true },
      { pubkey: accounts.lendingMarket, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: KLEND_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([sighash("kamino_init"), KAMINO_ADAPTER_ID]),
  });
}

function ixInitObligationFarm() {
  return initObligationFarmsForReserve(
    { mode: 0 },
    {
      payer: user.publicKey,
      owner: STATE_PDA,
      obligation: accounts.obligation,
      lendingMarketAuthority: accounts.lendingMarketAuthority,
      reserve: accounts.reserve,
      reserveFarmState: accounts.reserveFarmState,
      obligationFarm: accounts.obligationFarmState,
      lendingMarket: accounts.lendingMarket,
      farmsProgram: FARMS_PROGRAM_ID,
      rent: SYSVAR_RENT_PUBKEY,
      systemProgram: SystemProgram.programId,
    },
    KLEND_PROGRAM_ID,
  );
}

function ixRefreshReserve() {
  return refreshReserve(
    {
      reserve: accounts.reserve,
      lendingMarket: accounts.lendingMarket,
      pythOracle: KLEND_PROGRAM_ID,
      switchboardPriceOracle: KLEND_PROGRAM_ID,
      switchboardTwapOracle: KLEND_PROGRAM_ID,
      scopePrices: accounts.scopePrices,
    },
    KLEND_PROGRAM_ID,
  );
}

function ixRefreshObligation({ includeDepositReserve = false } = {}) {
  const ix = refreshObligation(
    {
      lendingMarket: accounts.lendingMarket,
      obligation: accounts.obligation,
    },
    KLEND_PROGRAM_ID,
  );
  if (includeDepositReserve) {
    ix.keys.push({ pubkey: accounts.reserve, isSigner: false, isWritable: false });
  }
  return ix;
}

function ixRefreshObligationFarm() {
  return refreshObligationFarmsForReserve(
    { mode: 0 },
    {
      crank: user.publicKey,
      obligation: accounts.obligation,
      lendingMarketAuthority: accounts.lendingMarketAuthority,
      reserve: accounts.reserve,
      reserveFarmState: accounts.reserveFarmState,
      obligationFarmUserState: accounts.obligationFarmState,
      lendingMarket: accounts.lendingMarket,
      farmsProgram: FARMS_PROGRAM_ID,
      rent: SYSVAR_RENT_PUBKEY,
      systemProgram: SystemProgram.programId,
    },
    KLEND_PROGRAM_ID,
  );
}

function ixKaminoDeposit() {
  const data = Buffer.alloc(8 + 32 + 8 + 8);
  sighash("kamino_deposit").copy(data, 0);
  KAMINO_ADAPTER_ID.copy(data, 8);
  data.writeBigUInt64LE(DEPOSIT_AMOUNT, 40);
  data.writeBigUInt64LE(0n, 48);
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [...adapterCpiFixedKeys(), ...kaminoDepositRemainingKeys()],
    data,
  });
}

function ixCurrentValueCpi() {
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [
      ...adapterCpiFixedKeys(),
      { pubkey: accounts.reserve, isSigner: false, isWritable: false },
      { pubkey: accounts.obligation, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([sighash("current_value_cpi"), KAMINO_ADAPTER_ID]),
  });
}

function ixKaminoWithdraw(shares) {
  const data = Buffer.alloc(8 + 32 + 8 + 8);
  sighash("kamino_withdraw").copy(data, 0);
  KAMINO_ADAPTER_ID.copy(data, 8);
  data.writeBigUInt64LE(shares, 40);
  data.writeBigUInt64LE(0n, 48);
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM_ID,
    keys: [...adapterCpiFixedKeys(), ...kaminoWithdrawRemainingKeys()],
    data,
  });
}

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

function kaminoDepositRemainingKeys() {
  return [
    { pubkey: STATE_PDA, isSigner: false, isWritable: true },
    { pubkey: accounts.obligation, isSigner: false, isWritable: true },
    { pubkey: accounts.lendingMarket, isSigner: false, isWritable: false },
    { pubkey: accounts.lendingMarketAuthority, isSigner: false, isWritable: false },
    { pubkey: accounts.reserve, isSigner: false, isWritable: true },
    { pubkey: USDC_MINT, isSigner: false, isWritable: false },
    { pubkey: accounts.reserveLiquiditySupply, isSigner: false, isWritable: true },
    { pubkey: accounts.reserveCollateralMint, isSigner: false, isWritable: true },
    { pubkey: accounts.reserveDestinationCollateral, isSigner: false, isWritable: true },
    { pubkey: adapterVault, isSigner: false, isWritable: true },
    { pubkey: KLEND_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: accounts.obligationFarmState, isSigner: false, isWritable: true },
    { pubkey: accounts.reserveFarmState, isSigner: false, isWritable: true },
    { pubkey: FARMS_PROGRAM_ID, isSigner: false, isWritable: false },
  ];
}

function kaminoWithdrawRemainingKeys() {
  return [
    { pubkey: STATE_PDA, isSigner: false, isWritable: true },
    { pubkey: accounts.obligation, isSigner: false, isWritable: true },
    { pubkey: accounts.lendingMarket, isSigner: false, isWritable: false },
    { pubkey: accounts.lendingMarketAuthority, isSigner: false, isWritable: false },
    { pubkey: accounts.reserve, isSigner: false, isWritable: true },
    { pubkey: USDC_MINT, isSigner: false, isWritable: false },
    { pubkey: accounts.reserveDestinationCollateral, isSigner: false, isWritable: true },
    { pubkey: accounts.reserveCollateralMint, isSigner: false, isWritable: true },
    { pubkey: accounts.reserveLiquiditySupply, isSigner: false, isWritable: true },
    { pubkey: adapterVault, isSigner: false, isWritable: true },
    { pubkey: KLEND_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: accounts.obligationFarmState, isSigner: false, isWritable: true },
    { pubkey: accounts.reserveFarmState, isSigner: false, isWritable: true },
    { pubkey: FARMS_PROGRAM_ID, isSigner: false, isWritable: false },
  ];
}

async function balances(connection) {
  return {
    userUsdc: await tokenAmount(connection, userUsdc),
    adapterVault: await tokenAmount(connection, adapterVault),
    reserveLiquiditySupply: await tokenAmount(connection, accounts.reserveLiquiditySupply),
    reserveDestinationCollateral: await tokenAmount(connection, accounts.reserveDestinationCollateral),
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

async function obligationCollateral(connection) {
  const info = await connection.getAccountInfo(accounts.obligation);
  if (!info) return null;
  const obligation = Obligation.decode(info.data);
  const deposit = obligation.deposits.find((item) => item.depositReserve.equals(accounts.reserve));
  return deposit?.depositedAmount?.toString() ?? "0";
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

function adapterId(label) {
  return createHash("sha256").update(`solana-yield-adapter:${label}`).digest();
}

function pk(value) {
  return new PublicKey(value);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
