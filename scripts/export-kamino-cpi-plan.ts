import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import {
  kaminoCpiAccountPlan,
  type DerivedKaminoAccounts,
} from "../packages/sdk/src/kaminoCpiPlan.js";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const derivedPath = path.resolve(
  repoRoot,
  process.env.KAMINO_DERIVED_ACCOUNTS ?? "docs/kamino-derived-accounts.json",
);
const outPath = path.resolve(
  repoRoot,
  process.env.KAMINO_CPI_PLAN_OUT ?? "packages/sdk/fixtures/kamino-cpi-account-plan.json",
);

const derived = JSON.parse(fs.readFileSync(derivedPath, "utf8")) as DerivedKaminoAccounts;
const fixture = {
  schema: "kamino-cpi-account-plan/v1",
  source: toRepoPath(derivedPath),
  generatedBy: "packages/sdk/src/kaminoCpiPlan.ts:kaminoCpiAccountPlan",
  rustBridge:
    "Use accountPlan.plans.*.accounts as canonical Kamino remaining-account plans after AdapterCpiRoute fixed accounts; this fixture does not imply live CPI is implemented.",
  accountPlan: kaminoCpiAccountPlan({ derived }),
};

fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(outPath, `${JSON.stringify(fixture, null, 2)}\n`);
console.log(`wrote ${toRepoPath(outPath)}`);

function toRepoPath(value: string): string {
  return path.relative(repoRoot, value).replaceAll("\\", "/");
}
