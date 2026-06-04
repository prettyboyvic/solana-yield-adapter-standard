use serde_json::{Map, Value};

const FIXTURE_JSON: &str =
    include_str!("../../../packages/sdk/fixtures/kamino-cpi-account-plan.json");
const EXPECTED_PLANS: [&str; 7] = [
    "initUserMetadata",
    "initObligation",
    "refreshReserve",
    "refreshObligation",
    "refreshObligationFarmsForReserve",
    "depositReserveLiquidityAndObligationCollateralV2",
    "withdrawObligationCollateralAndRedeemReserveCollateralV2",
];

#[test]
fn kamino_fixture_defines_all_canonical_plan_sections() {
    let fixture = fixture();
    assert_eq!(
        fixture["schema"], "kamino-cpi-account-plan/v1",
        "unexpected fixture schema"
    );
    assert!(
        fixture["rustBridge"]
            .as_str()
            .expect("rustBridge string")
            .contains("does not imply live CPI is implemented"),
        "fixture must keep live-CPI status explicit"
    );

    let plans = plans(&fixture);
    for expected in EXPECTED_PLANS {
        let plan = plans
            .get(expected)
            .unwrap_or_else(|| panic!("missing {expected}"));
        assert_eq!(
            plan["instruction"], expected,
            "{expected} instruction field must match its plan key"
        );
        assert!(
            !accounts(plan).is_empty(),
            "{expected} must contain at least one account"
        );
    }
}

#[test]
fn kamino_fixture_names_liquidity_accounts_for_deposit_and_withdraw() {
    let fixture = fixture();
    let plans = plans(&fixture);

    assert!(
        has_account(
            plans
                .get("depositReserveLiquidityAndObligationCollateralV2")
                .expect("deposit plan"),
            "userSourceLiquidity"
        ),
        "deposit plan must name Kamino userSourceLiquidity"
    );
    assert!(
        has_account(
            plans
                .get("withdrawObligationCollateralAndRedeemReserveCollateralV2")
                .expect("withdraw plan"),
            "userDestinationLiquidity"
        ),
        "withdraw plan must name Kamino userDestinationLiquidity"
    );
}

#[test]
fn kamino_fixture_has_no_empty_pubkeys() {
    let fixture = fixture();
    for (plan_name, account) in all_accounts(&fixture) {
        let account_name = account_name(account);
        assert!(
            !account_pubkey(account).trim().is_empty(),
            "{plan_name}.{account_name} has an empty pubkey"
        );
    }
}

#[test]
fn kamino_fixture_uses_one_optional_none_placeholder() {
    let fixture = fixture();
    let placeholder = optional_none_placeholder(&fixture);
    let mut none_count = 0;

    for (plan_name, account) in all_accounts(&fixture) {
        if account["isNone"].as_bool() == Some(true) {
            none_count += 1;
            assert_eq!(
                account["optional"].as_bool(),
                Some(true),
                "{plan_name}.{} isNone without optional=true",
                account_name(account)
            );
            assert_eq!(
                account_pubkey(account),
                placeholder,
                "{plan_name}.{} uses a different optional-none placeholder",
                account_name(account)
            );
        }
    }

    assert!(
        none_count > 0,
        "fixture must contain optional-none accounts"
    );
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE_JSON).expect("valid Kamino CPI account-plan fixture")
}

fn plans(fixture: &Value) -> &Map<String, Value> {
    fixture
        .pointer("/accountPlan/plans")
        .and_then(Value::as_object)
        .expect("accountPlan.plans object")
}

fn accounts(plan: &Value) -> &Vec<Value> {
    plan["accounts"].as_array().expect("plan accounts array")
}

fn has_account(plan: &Value, name: &str) -> bool {
    accounts(plan)
        .iter()
        .any(|account| account["name"].as_str() == Some(name))
}

fn all_accounts(fixture: &Value) -> impl Iterator<Item = (&str, &Value)> {
    plans(fixture).iter().flat_map(|(plan_name, plan)| {
        accounts(plan)
            .iter()
            .map(move |account| (plan_name.as_str(), account))
    })
}

fn optional_none_placeholder(fixture: &Value) -> &str {
    all_accounts(fixture)
        .find_map(|(_, account)| {
            (account["isNone"].as_bool() == Some(true)).then(|| account_pubkey(account))
        })
        .expect("at least one optional-none account")
}

fn account_name(account: &Value) -> &str {
    account["name"].as_str().expect("account name")
}

fn account_pubkey(account: &Value) -> &str {
    account["pubkey"].as_str().expect("account pubkey")
}
