#!/usr/bin/env node
//
// Jupiter Perps USDC -> JLP live mainnet-fork roundtrip:
//   initialize_adapter -> jupiter_deposit -> current_value_cpi -> jupiter_withdraw
//
// The program/account map is the committed on-chain-IDL-derived Jupiter fixture.
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
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
} from "@solana/web3.js";
import {
  AccountLayout,
  TOKEN_PROGRAM_ID,
  getAccount,
  getAssociatedTokenAddressSync,
  getMint,
} from "@solana/spl-token";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixture = JSON.parse(
  fs.readFileSync(path.join(repoRoot, "packages/sdk/fixtures/jupiter-cpi-account-plan.json"), "utf8"),
);

const RPC_URL = process.env.MAINNET_RPC_URL ?? "https://api.mainnet-beta.solana.com";
const LOCAL_URL = process.env.LOCAL_RPC_URL ?? "http://127.0.0.1:8899";
const DEPOSIT_AMOUNT = BigInt(process.env.JUPITER_DEPOSIT_LAMPORTS ?? "1000000");
const COMPUTE_UNITS = Number(process.env.JUPITER_COMPUTE_UNITS ?? "1400000");
const LEDGER = path.resolve(
  repoRoot,
  process.env.JUPITER_FORK_LEDGER ?? "target/jupiter-mainnet-fork-ledger",
);
const FIXTURE_DIR = path.resolve(repoRoot, "target/jupiter-mainnet-fork-fixtures");
const LOG_PATH = path.join(FIXTURE_DIR, "validator.log");
const EVIDENCE_PATH = path.join(FIXTURE_DIR, "evidence.json");
// Preserve mainnet Doves AG bytes but move publish_time past validator startup
// and the pinned-slot clock jump. The exact patch is recorded in evidence.json.
const DOVES_AG_PUBLISH_TIME_OFFSET = 177;
const DOVES_AG_FORWARD_SECONDS = Number(process.env.JUPITER_DOVES_AG_FORWARD_SECONDS ?? "60000");

const JUPITER_PROGRAM = pk(fixture.program);
const REFERENCE_ADAPTER_PROGRAM = pk("BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1");
const DISPATCHER_PROGRAM = pk("37fdMFG3eh91i7WYk4MgwYBGqoXK4dbpV73UUh6uxvtY");
const USDC_MINT = pk(fixture.underlyingMint);
const JLP_MINT = pk(fixture.receiptMint);
const POOL = pk(fixture.pool);
const DOVES_PROGRAM = pk("DoVEsk76QybCEHQGzkvYPWLQu9gzNoZZZt3TPiL597e");
const JUPITER_ADAPTER_ID = Buffer.from(fixture.adapterCustody.adapterIdHex, "hex");
const PROTOCOL_JUPITER_LP = 3;
const dovesAgAccounts = fixture.aumRemainingAccounts
  .filter((account) => account.name.startsWith("aumDovesAgPriceAccount"))
  .map((account) => pk(account.pubkey));
const dovesAgKeys = new Set(dovesAgAccounts.map((account) => account.toBase58()));
const cloneAccounts = fixture.cloneAccounts.map(pk).filter(
  (account) => !dovesAgKeys.has(account.toBase58()),
);

const user = Keypair.generate();
const [statePda, stateBump] = PublicKey.findProgramAddressSync(
  [Buffer.from("adapter"), JUPITER_ADAPTER_ID],
  REFERENCE_ADAPTER_PROGRAM,
);
const [positionPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("position"), JUPITER_ADAPTER_ID, user.publicKey.toBuffer()],
  REFERENCE_ADAPTER_PROGRAM,
);
const userUsdc = getAssociatedTokenAddressSync(USDC_MINT, user.publicKey);
const adapterUsdc = getAssociatedTokenAddressSync(USDC_MINT, statePda, true);
const adapterJlp = getAssociatedTokenAddressSync(JLP_MINT, statePda, true);

assertEqual("adapter state", statePda.toBase58(), fixture.adapterCustody.adapterState);
assertEqual("adapter USDC", adapterUsdc.toBase58(), fixture.adapterCustody.adapterUnderlyingVault);
assertEqual("adapter JLP", adapterJlp.toBase58(), fixture.adapterCustody.adapterJlpVault);

let validator;
let validatorExit;
const started = Date.now();
try {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  writeSystemAccountFixture(path.join(FIXTURE_DIR, "user-sol.json"), user.publicKey, 100n * 1_000_000_000n);
  writeTokenAccountFixture(
    path.join(FIXTURE_DIR, "user-usdc.json"),
    userUsdc,
    USDC_MINT,
    user.publicKey,
    DEPOSIT_AMOUNT * 2n,
  );
  writeTokenAccountFixture(
    path.join(FIXTURE_DIR, "adapter-usdc.json"),
    adapterUsdc,
    USDC_MINT,
    statePda,
    0n,
  );
  writeTokenAccountFixture(
    path.join(FIXTURE_DIR, "adapter-jlp.json"),
    adapterJlp,
    JLP_MINT,
    statePda,
    0n,
  );

  const mainnet = new Connection(RPC_URL, "confirmed");
  const forkSlot = Number(process.env.JUPITER_FORK_SLOT ?? (await mainnet.getSlot("finalized")));
  const dovesAgFixtures = await writeForwardedDovesAgFixtures(mainnet);
  const args = validatorArgsFor(forkSlot, dovesAgFixtures);
  console.log("== Jupiter LP mainnet-fork roundtrip ==");
  console.log(`rpc=${RPC_URL}`);
  console.log(`forkSlot=${forkSlot}`);
  console.log(`user=${user.publicKey.toBase58()}`);
  console.log(`state=${statePda.toBase58()} bump=${stateBump}`);
  console.log(`position=${positionPda.toBase58()}`);
  console.log(`adapterUsdc=${adapterUsdc.toBase58()}`);
  console.log(`adapterJlp=${adapterJlp.toBase58()}`);
  console.log(`depositAmount=${DEPOSIT_AMOUNT}`);
  console.log(`dovesAgPublishTimeForwardSeconds=${DOVES_AG_FORWARD_SECONDS}`);
  console.log(`validatorCommand=${formatPowerShellCommand(args)}`);

  const validatorBin = process.platform === "win32" ? "solana-test-validator.exe" : "solana-test-validator";
  validator = spawn(validatorBin, args, { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
  validatorExit = new Promise((resolve) => {
    validator.once("exit", (code, signal) => resolve({ code, signal }));
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
  const poolInfo = await connection.getAccountInfo(POOL);
  const jlpMint = await getMint(connection, JLP_MINT);
  if (!poolInfo) throw new Error("forked Jupiter Pool is missing");
  const aumBefore = readPoolAumUsd(poolInfo.data);
  const expectedJlp = (DEPOSIT_AMOUNT * jlpMint.supply) / aumBefore;
  const minJlpOut = (expectedJlp * 9n) / 10n;
  if (minJlpOut <= 0n) throw new Error(`computed min JLP floor is zero: expected=${expectedJlp}`);

  const evidence = {
    forkSlot,
    localRpc: LOCAL_URL,
    user: user.publicKey.toBase58(),
    state: statePda.toBase58(),
    position: positionPda.toBase58(),
    userUsdc: userUsdc.toBase58(),
    adapterUsdc: adapterUsdc.toBase58(),
    adapterJlp: adapterJlp.toBase58(),
    depositAmount: DEPOSIT_AMOUNT.toString(),
    expectedJlpBeforeFees: expectedJlp.toString(),
    minJlpOut: minJlpOut.toString(),
    dovesAgPublishTimeFixtures: dovesAgFixtures.map(({ file, ...fixtureEvidence }) => fixtureEvidence),
    signatures: {},
    computeUnits: {},
    balances: {},
    stateFields: {},
    positionFields: {},
  };

  evidence.balances.before = await balances(connection);
  await sendLabeled(connection, evidence, "initializeAdapter", [ixInitializeAdapter()], []);
  await sendLabeled(connection, evidence, "deposit", [ixJupiterDeposit(minJlpOut)], []);
  evidence.balances.afterDeposit = await balances(connection);
  evidence.stateFields.afterDeposit = await adapterState(connection);
  evidence.positionFields.afterDeposit = await positionState(connection);

  const minted = BigInt(evidence.balances.afterDeposit.adapterJlp ?? "0");
  if (minted < minJlpOut) throw new Error(`deposit minted ${minted}, below floor ${minJlpOut}`);
  if (BigInt(evidence.positionFields.afterDeposit?.shares ?? "0") !== minted) {
    throw new Error("position shares do not equal minted JLP");
  }

  await sendLabeled(connection, evidence, "currentValue", [ixCurrentValueCpi()], []);
  evidence.stateFields.afterCurrentValue = await adapterState(connection);
  evidence.positionFields.afterCurrentValue = await positionState(connection);
  const currentValue = BigInt(evidence.stateFields.afterCurrentValue?.totalAssets ?? "0");
  if (currentValue <= 0n) throw new Error(`current_value returned ${currentValue}`);
  const minAssetsOut = (currentValue * 9n) / 10n;
  if (minAssetsOut <= 0n) throw new Error("computed withdraw floor is zero");
  evidence.currentValueAssets = currentValue.toString();
  evidence.minAssetsOut = minAssetsOut.toString();

  await sendLabeled(connection, evidence, "withdraw", [ixJupiterWithdraw(minted, minAssetsOut)], []);
  evidence.balances.afterWithdraw = await balances(connection);
  evidence.stateFields.afterWithdraw = await adapterState(connection);
  evidence.positionFields.afterWithdraw = await positionState(connection);

  const userBefore = BigInt(evidence.balances.before.userUsdc ?? "0");
  const userAfterDeposit = BigInt(evidence.balances.afterDeposit.userUsdc ?? "0");
  const userAfterWithdraw = BigInt(evidence.balances.afterWithdraw.userUsdc ?? "0");
  if (userAfterDeposit !== userBefore - DEPOSIT_AMOUNT) {
    throw new Error(`deposit user delta mismatch: before=${userBefore} after=${userAfterDeposit}`);
  }
  if (userAfterWithdraw <= userAfterDeposit) throw new Error("withdraw returned no USDC");
  if (BigInt(evidence.balances.afterWithdraw.adapterJlp ?? "-1") !== 0n) {
    throw new Error("full withdraw left JLP in adapter vault");
  }
  if (BigInt(evidence.balances.afterWithdraw.adapterUsdc ?? "-1") !== 0n) {
    throw new Error("full withdraw left USDC in adapter vault");
  }
  if (
    BigInt(evidence.stateFields.afterWithdraw?.totalAssets ?? "-1") !== 0n ||
    BigInt(evidence.stateFields.afterWithdraw?.totalShares ?? "-1") !== 0n ||
    BigInt(evidence.positionFields.afterWithdraw?.shares ?? "-1") !== 0n
  ) {
    throw new Error("full withdraw did not zero adapter and position accounting");
  }
  evidence.redeemedToUser = (userAfterWithdraw - userAfterDeposit).toString();
  evidence.finalUserDeltaVsStart = (userAfterWithdraw - userBefore).toString();
  evidence.elapsedMs = Date.now() - started;

  writeJson(EVIDENCE_PATH, evidence);
  console.log("\n== Evidence ==");
  console.log(JSON.stringify(evidence, null, 2));
  console.log(`\nevidenceFile=${EVIDENCE_PATH}`);
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

function validatorArgsFor(forkSlot, dovesAgFixtures) {
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
    JUPITER_PROGRAM.toBase58(),
    "--bpf-program",
    REFERENCE_ADAPTER_PROGRAM.toBase58(),
    referenceSo,
    "--bpf-program",
    DISPATCHER_PROGRAM.toBase58(),
    dispatcherSo,
    ...cloneAccounts.flatMap((account) => ["--clone", account.toBase58()]),
    ...dovesAgFixtures.flatMap(({ pubkey, file }) => ["--account", pubkey, file]),
    "--account",
    user.publicKey.toBase58(),
    path.join(FIXTURE_DIR, "user-sol.json"),
    "--account",
    userUsdc.toBase58(),
    path.join(FIXTURE_DIR, "user-usdc.json"),
    "--account",
    adapterUsdc.toBase58(),
    path.join(FIXTURE_DIR, "adapter-usdc.json"),
    "--account",
    adapterJlp.toBase58(),
    path.join(FIXTURE_DIR, "adapter-jlp.json"),
  ];
}

function ixInitializeAdapter() {
  const metadata = Buffer.from("ipfs://solana-yield-adapters/jupiter-lp.json", "utf8");
  const data = Buffer.alloc(8 + 32 + 1 + 32 * 4 + 4 + metadata.length);
  let offset = 0;
  sighash("initialize_adapter").copy(data, offset); offset += 8;
  JUPITER_ADAPTER_ID.copy(data, offset); offset += 32;
  data.writeUInt8(PROTOCOL_JUPITER_LP, offset); offset += 1;
  USDC_MINT.toBuffer().copy(data, offset); offset += 32;
  JLP_MINT.toBuffer().copy(data, offset); offset += 32;
  POOL.toBuffer().copy(data, offset); offset += 32;
  PublicKey.default.toBuffer().copy(data, offset); offset += 32;
  data.writeUInt32LE(metadata.length, offset); offset += 4;
  metadata.copy(data, offset);
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM,
    keys: [
      { pubkey: statePda, isSigner: false, isWritable: true },
      { pubkey: user.publicKey, isSigner: true, isWritable: true },
      { pubkey: user.publicKey, isSigner: true, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });
}

function ixJupiterDeposit(minJlpOut) {
  return adapterIx("jupiter_deposit", DEPOSIT_AMOUNT, minJlpOut, liquidityRemaining("addLiquidity2"));
}

function ixJupiterWithdraw(shares, minAssetsOut) {
  return adapterIx("jupiter_withdraw", shares, minAssetsOut, liquidityRemaining("removeLiquidity2"));
}

function ixCurrentValueCpi() {
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM,
    keys: [
      ...adapterFixedKeys(),
      { pubkey: POOL, isSigner: false, isWritable: false },
      { pubkey: JLP_MINT, isSigner: false, isWritable: false },
      { pubkey: adapterJlp, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([sighash("current_value_cpi"), JUPITER_ADAPTER_ID]),
  });
}

function adapterIx(name, first, second, remaining) {
  const data = Buffer.alloc(56);
  sighash(name).copy(data, 0);
  JUPITER_ADAPTER_ID.copy(data, 8);
  data.writeBigUInt64LE(first, 40);
  data.writeBigUInt64LE(second, 48);
  return new TransactionInstruction({
    programId: REFERENCE_ADAPTER_PROGRAM,
    keys: [...adapterFixedKeys(), ...remaining],
    data,
  });
}

function adapterFixedKeys() {
  return [
    { pubkey: user.publicKey, isSigner: true, isWritable: true },
    { pubkey: statePda, isSigner: false, isWritable: true },
    { pubkey: positionPda, isSigner: false, isWritable: true },
    { pubkey: userUsdc, isSigner: false, isWritable: true },
    { pubkey: adapterUsdc, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ];
}

function liquidityRemaining(planName) {
  return [
    ...fixture.plans[planName].accounts.map((account, index) => ({
      pubkey: pk(account.pubkey),
      isSigner: false,
      isWritable: index === 0 ? true : account.isWritable,
    })),
    ...fixture.aumRemainingAccounts.map((account) => ({
      pubkey: pk(account.pubkey),
      isSigner: false,
      isWritable: account.isWritable,
    })),
  ];
}

async function sendLabeled(connection, evidence, label, instructions, extraSigners) {
  console.log(`${label}: sending`);
  const tx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: COMPUTE_UNITS }),
    ...instructions,
  );
  try {
    const { blockhash, lastValidBlockHeight } = await whileValidatorAlive(
      connection.getLatestBlockhash("confirmed"),
      `${label} blockhash`,
    );
    tx.feePayer = user.publicKey;
    tx.recentBlockhash = blockhash;
    tx.sign(user, ...extraSigners);
    const signature = await withTimeout(
      whileValidatorAlive(
        connection.sendRawTransaction(tx.serialize(), {
          skipPreflight: false,
          preflightCommitment: "confirmed",
        }),
        `${label} send`,
      ),
      90_000,
      `${label} send timed out`,
    );
    await waitForSignature(connection, signature, label, lastValidBlockHeight);
    evidence.signatures[label] = signature;
    const details = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    evidence.computeUnits[label] = details?.meta?.computeUnitsConsumed ?? null;
    evidence[`${label}Logs`] = details?.meta?.logMessages ?? [];
    console.log(`${label}: ${signature} CU=${evidence.computeUnits[label]}`);
  } catch (error) {
    if (typeof error?.getLogs === "function") {
      try {
        console.error((await error.getLogs(connection)).join("\n"));
      } catch {
        // Ignore log-fetch failures; the original transaction error is primary.
      }
    }
    throw error;
  }
}

async function waitForSignature(connection, signature, label, lastValidBlockHeight) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < 90_000) {
    const response = await withTimeout(
      whileValidatorAlive(
        connection.getSignatureStatuses([signature], { searchTransactionHistory: true }),
        `${label} status`,
      ),
      5_000,
      `${label} status request timed out`,
    );
    const status = response.value[0];
    if (status?.err) throw new Error(`${label} failed: ${JSON.stringify(status.err)}`);
    if (status?.confirmationStatus === "confirmed" || status?.confirmationStatus === "finalized") {
      return;
    }
    const blockHeight = await withTimeout(
      whileValidatorAlive(connection.getBlockHeight("confirmed"), `${label} block height`),
      5_000,
      `${label} block-height request timed out`,
    );
    if (blockHeight > lastValidBlockHeight) throw new Error(`${label} blockhash expired`);
    await sleep(500);
  }
  throw new Error(`${label} confirmation timed out`);
}

function whileValidatorAlive(promise, label) {
  return Promise.race([
    promise,
    validatorExit.then(({ code, signal }) => {
      throw new Error(`validator exited during ${label}: code=${code} signal=${signal}`);
    }),
  ]);
}

async function balances(connection) {
  const mint = await getMint(connection, JLP_MINT);
  return {
    userUsdc: await tokenAmount(connection, userUsdc),
    adapterUsdc: await tokenAmount(connection, adapterUsdc),
    adapterJlp: await tokenAmount(connection, adapterJlp),
    custodyUsdc: await tokenAmount(connection, pk(fixture.plans.addLiquidity2.accounts[9].pubkey)),
    jlpMintSupply: mint.supply.toString(),
  };
}

async function tokenAmount(connection, address) {
  try {
    return (await getAccount(connection, address)).amount.toString();
  } catch {
    return null;
  }
}

async function adapterState(connection) {
  const info = await connection.getAccountInfo(statePda);
  if (!info) return null;
  return {
    totalAssets: info.data.readBigUInt64LE(201).toString(),
    totalShares: info.data.readBigUInt64LE(209).toString(),
    lastUpdateSlot: info.data.readBigUInt64LE(217).toString(),
  };
}

async function positionState(connection) {
  const info = await connection.getAccountInfo(positionPda);
  if (!info) return null;
  return {
    shares: info.data.readBigUInt64LE(72).toString(),
    principalAssets: info.data.readBigUInt64LE(80).toString(),
    lastValueAssets: info.data.readBigUInt64LE(88).toString(),
  };
}

function readPoolAumUsd(data) {
  const nameLength = data.readUInt32LE(8);
  const custodyCountOffset = 12 + nameLength;
  const custodyCount = data.readUInt32LE(custodyCountOffset);
  const aumOffset = custodyCountOffset + 4 + custodyCount * 32;
  return data.readBigUInt64LE(aumOffset) + (data.readBigUInt64LE(aumOffset + 8) << 64n);
}

function writeSystemAccountFixture(file, pubkey, lamports) {
  writeJson(file, {
    pubkey: pubkey.toBase58(),
    account: {
      lamports: Number(lamports),
      data: ["", "base64"],
      owner: SystemProgram.programId.toBase58(),
      executable: false,
      rentEpoch: 0,
      space: 0,
    },
  });
}

function writeTokenAccountFixture(file, pubkey, mint, owner, amount) {
  const data = Buffer.alloc(AccountLayout.span);
  AccountLayout.encode(
    {
      mint,
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
  writeJson(file, {
    pubkey: pubkey.toBase58(),
    account: {
      lamports: 2039280,
      data: [data.toString("base64"), "base64"],
      owner: TOKEN_PROGRAM_ID.toBase58(),
      executable: false,
      rentEpoch: 0,
      space: AccountLayout.span,
    },
  });
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

async function writeForwardedDovesAgFixtures(connection) {
  if (!Number.isSafeInteger(DOVES_AG_FORWARD_SECONDS) || DOVES_AG_FORWARD_SECONDS <= 0) {
    throw new Error(`invalid JUPITER_DOVES_AG_FORWARD_SECONDS: ${DOVES_AG_FORWARD_SECONDS}`);
  }
  const infos = await connection.getMultipleAccountsInfo(dovesAgAccounts, "confirmed");
  const now = Math.floor(Date.now() / 1000);
  const forwardedPublishTime = BigInt(now + DOVES_AG_FORWARD_SECONDS);
  return dovesAgAccounts.map((pubkey, index) => {
    const info = infos[index];
    if (!info) throw new Error(`mainnet Doves AG account is missing: ${pubkey.toBase58()}`);
    const data = Buffer.from(info.data);
    if (!info.owner.equals(DOVES_PROGRAM) || data.length !== 394) {
      throw new Error(`unexpected Doves AG account owner or layout: ${pubkey.toBase58()}`);
    }
    const observedPublishTime = data.readBigInt64LE(DOVES_AG_PUBLISH_TIME_OFFSET);
    if (
      observedPublishTime < BigInt(now - 7 * 24 * 3600) ||
      observedPublishTime > BigInt(now + 3600)
    ) {
      throw new Error(
        `unexpected Doves AG publish time at offset ${DOVES_AG_PUBLISH_TIME_OFFSET}: ` +
          `${pubkey.toBase58()}=${observedPublishTime}`,
      );
    }
    data.writeBigInt64LE(forwardedPublishTime, DOVES_AG_PUBLISH_TIME_OFFSET);
    const file = path.join(FIXTURE_DIR, `doves-ag-${index}.json`);
    writeJson(file, {
      pubkey: pubkey.toBase58(),
      account: {
        lamports: info.lamports,
        data: [data.toString("base64"), "base64"],
        owner: info.owner.toBase58(),
        executable: info.executable,
        rentEpoch: 0,
        space: data.length,
      },
    });
    return {
      pubkey: pubkey.toBase58(),
      file,
      publishTimeOffset: DOVES_AG_PUBLISH_TIME_OFFSET,
      observedPublishTime: observedPublishTime.toString(),
      forwardedPublishTime: forwardedPublishTime.toString(),
    };
  });
}

async function waitForValidator(connection, child) {
  const startedAt = Date.now();
  let exit;
  child.once("exit", (code, signal) => {
    exit = { code, signal };
  });
  while (Date.now() - startedAt < 120_000) {
    if (exit) throw new Error(`validator exited before RPC ready: ${JSON.stringify(exit)}`);
    try {
      await withTimeout(connection.getVersion(), 5_000, "validator getVersion timed out");
      await withTimeout(
        connection.getLatestBlockhash("processed"),
        5_000,
        "validator getLatestBlockhash timed out",
      );
      return;
    } catch {
      await sleep(1000);
    }
  }
  throw new Error("validator RPC did not become ready within 120s");
}

function withTimeout(promise, ms, message) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(message)), ms);
    Promise.resolve(promise).then(
      (value) => {
        clearTimeout(timeout);
        resolve(value);
      },
      (error) => {
        clearTimeout(timeout);
        reject(error);
      },
    );
  });
}

function sighash(name) {
  return createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

function formatPowerShellCommand(args) {
  return ["solana-test-validator", ...args]
    .map((part) => (/[\s;]/.test(part) ? `"${part.replaceAll('"', '`"')}"` : part))
    .join(" ");
}

function assertEqual(label, actual, expected) {
  if (actual !== expected) throw new Error(`${label} mismatch: actual=${actual} expected=${expected}`);
}

function pk(value) {
  return new PublicKey(value);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
