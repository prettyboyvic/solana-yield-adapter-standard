import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import { createRequire } from "node:module";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { Connection, PublicKey } from "@solana/web3.js";

const require = createRequire(import.meta.url);
const Decimal = require("decimal.js");
const { Reserve, Obligation } = require("@kamino-finance/klend-sdk/dist/idl_codegen/accounts");
const { KaminoReserve, DEFAULT_RECENT_SLOT_DURATION_MS } = require(
  "@kamino-finance/klend-sdk/dist/classes/reserve",
);
const klendPackage = require("@kamino-finance/klend-sdk/package.json");

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixturePath = path.resolve(
  repoRoot,
  process.argv[2] ?? "tests/fixtures/kamino-current-value-424277911.json",
);

const fixture = JSON.parse(fs.readFileSync(fixturePath, "utf8"));
if (fixture.klendSdk.version !== "3.2.26" || klendPackage.version !== "3.2.26") {
  throw new Error(
    `expected @kamino-finance/klend-sdk@3.2.26, fixture=${fixture.klendSdk.version}, installed=${klendPackage.version}`,
  );
}

const reserveData = Buffer.from(fixture.accounts.reserve.dataBase64, "base64");
const obligationData = Buffer.from(fixture.accounts.oracleObligation.dataBase64, "base64");
const reserveState = Reserve.decode(reserveData);
const obligationState = Obligation.decode(obligationData);
const reserve = new KaminoReserve(
  reserveState,
  new PublicKey(fixture.accounts.reserve.pubkey),
  { price: new Decimal(1), timestamp: new Decimal(0) },
  new Connection(fixture.rpc, "confirmed"),
  DEFAULT_RECENT_SLOT_DURATION_MS,
);

const reserveKey = new PublicKey(fixture.sdkOracle.reserve);
const deposit = obligationState.deposits.find((item: any) => item.depositReserve.equals(reserveKey));
if (!deposit) {
  throw new Error(`obligation ${fixture.sdkOracle.obligation} has no deposit for ${reserveKey.toBase58()}`);
}

const depositedCollateral = new Decimal(deposit.depositedAmount.toString());
const totalSupply = reserve.getTotalSupply();
const collateralMintTotalSupply = new Decimal(reserveState.collateral.mintTotalSupply.toString());
const sdkExpectedAssets = depositedCollateral.mul(totalSupply).div(collateralMintTotalSupply).floor();
const fixtureExpectedAssets = new Decimal(fixture.sdkOracle.expectedAssets);
if (!sdkExpectedAssets.eq(fixtureExpectedAssets)) {
  throw new Error(
    `fixture expectedAssets mismatch: sdk=${sdkExpectedAssets.toFixed(0)} fixture=${fixtureExpectedAssets.toFixed(0)}`,
  );
}

console.log(
  `[sdk] slot=${fixture.slot} sdkVersion=${klendPackage.version} adapterObligationFound=${fixture.accounts.adapterObligation.accountFound}`,
);
console.log(
  `[sdk] reserve=${fixture.sdkOracle.reserve} obligation=${fixture.sdkOracle.obligation} owner=${obligationState.owner.toBase58()}`,
);
console.log(
  `[sdk] depositedCollateral=${depositedCollateral.toFixed(0)} totalSupply=${totalSupply.toString()} collateralMintTotalSupply=${collateralMintTotalSupply.toFixed(0)} expectedAssets=${sdkExpectedAssets.toFixed(0)}`,
);

const args = [
  "test",
  "--quiet",
  "-p",
  "reference_yield_adapter",
  "--test",
  "kamino_current_value_fixture",
  "kamino_current_value_fixture_matches_sdk",
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
  `[match] rust and klend-sdk oracle agree within <=1 lamport at slot ${fixture.slot}`,
);
