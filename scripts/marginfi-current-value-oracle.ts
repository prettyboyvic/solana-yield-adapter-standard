import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import { createRequire } from "node:module";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { PublicKey } from "@solana/web3.js";

const require = createRequire(import.meta.url);
const BigNumber = require("bignumber.js");
const marginfi = require("@mrgnlabs/marginfi-client-v2");
const marginfiPackage = require("@mrgnlabs/marginfi-client-v2/package.json");
const { wrappedI80F48toBigNumber } = require("@mrgnlabs/mrgn-common");

const marginfiPackageRoot = path.dirname(
  require.resolve("@mrgnlabs/marginfi-client-v2/package.json"),
);
const { BorshCoder } = require(
  require.resolve("@coral-xyz/anchor", { paths: [marginfiPackageRoot] }),
);

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixturePath = path.resolve(
  repoRoot,
  process.argv[2] ?? "tests/fixtures/marginfi-current-value-424351480.json",
);
const fixture = JSON.parse(fs.readFileSync(fixturePath, "utf8"));

if (
  fixture.marginfiSdk.version !== "6.4.2" ||
  marginfiPackage.version !== "6.4.2"
) {
  throw new Error(
    `expected @mrgnlabs/marginfi-client-v2@6.4.2, fixture=${fixture.marginfiSdk.version}, installed=${marginfiPackage.version}`,
  );
}

const bankKey = new PublicKey(fixture.accounts.bank.pubkey);
const accountKey = new PublicKey(fixture.accounts.sampleMarginfiAccount.pubkey);
const bankData = Buffer.from(fixture.accounts.bank.dataBase64, "base64");
const accountData = Buffer.from(
  fixture.accounts.sampleMarginfiAccount.dataBase64,
  "base64",
);
const coder = new BorshCoder(marginfi.MARGINFI_IDL);
const bank = coder.accounts.decode("Bank", bankData);
const account = coder.accounts.decode("MarginfiAccount", accountData);
const balance = account.lending_account.balances.find(
  (item: any) =>
    (item.active === true || item.active === 1) && item.bank_pk.equals(bankKey),
);
if (!balance) {
  throw new Error(`MarginFi account ${accountKey.toBase58()} has no active USDC balance`);
}

const assetShares = wrappedI80F48toBigNumber(balance.asset_shares);
const assetShareValue = wrappedI80F48toBigNumber(bank.asset_share_value);
const sdkExactAssets = marginfi.getAssetQuantity({ assetShareValue }, assetShares);
const sdkExpectedAssets = sdkExactAssets.integerValue(BigNumber.ROUND_FLOOR);
if (!sdkExactAssets.eq(fixture.sdkOracle.exactAssets)) {
  throw new Error(
    `fixture exactAssets mismatch: sdk=${sdkExactAssets.toFixed()} fixture=${fixture.sdkOracle.exactAssets}`,
  );
}
const fixtureExpectedAssets = new BigNumber(fixture.sdkOracle.expectedAssets);
if (!sdkExpectedAssets.eq(fixtureExpectedAssets)) {
  throw new Error(
    `fixture expectedAssets mismatch: sdk=${sdkExpectedAssets.toFixed(0)} fixture=${fixtureExpectedAssets.toFixed(0)}`,
  );
}
if (account.authority.toBase58() !== fixture.sdkOracle.accountAuthority) {
  throw new Error("fixture MarginFi account authority mismatch");
}

console.log(
  `[sdk] slot=${fixture.slot} sdkVersion=${marginfiPackage.version} bank=${bankKey.toBase58()} account=${accountKey.toBase58()}`,
);
console.log(
  `[sdk] authority=${account.authority.toBase58()} assetShares=${assetShares.toFixed()} assetShareValue=${assetShareValue.toFixed()} expectedAssets=${sdkExpectedAssets.toFixed(0)}`,
);
console.log(
  `[sdk] oracle-independent underlying value; bankOracle=${bank.config.oracle_keys[0].toBase58()}`,
);

const args = [
  "test",
  "--quiet",
  "-p",
  "reference_yield_adapter",
  "--test",
  "marginfi_current_value_fixture",
  "marginfi_current_value_fixture_matches_sdk",
  "--",
  "--nocapture",
];
console.log(`[rust] cargo ${args.join(" ")}`);
const cargoBin = process.platform === "win32" ? "cargo.exe" : "cargo";
const result = spawnSync(cargoBin, args, {
  cwd: repoRoot,
  stdio: "inherit",
});
if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

console.log(
  `[match] Rust and MarginFi SDK oracle agree within <=1 lamport at slot ${fixture.slot}`,
);
