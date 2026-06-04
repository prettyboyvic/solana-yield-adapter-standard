//! Test-only validation that a positional slice of Kamino "remaining accounts"
//! matches the canonical plan committed in
//! `packages/sdk/fixtures/kamino-cpi-account-plan.json`.
//!
//! This file performs NO CPI and adds NO signer seeds. It only checks ordering
//! and signer/writable expectations against the fixture, which is the single
//! source of truth for Kamino remaining-account order (the order is read from
//! the fixture, never hardcoded here).
//!
//! Mapping to production: in the adapter route the Kamino accounts arrive as
//! `ctx.remaining_accounts: &[AccountInfo]` in a fixed positional order. Each
//! `AccountInfo` maps to:
//!     pubkey      <- info.key.to_string()
//!     is_signer   <- info.is_signer
//!     is_writable <- info.is_writable
//! A future production validator should compare `remaining_accounts` against
//! values stored in `AdapterState` (not against this fixture, which must not be
//! embedded in the on-chain program). This test only proves the canonical
//! order/flags are well-formed and that a correct slice validates.

use serde_json::Value;

const FIXTURE_JSON: &str =
    include_str!("../../../packages/sdk/fixtures/kamino-cpi-account-plan.json");

const DEPOSIT: &str = "depositReserveLiquidityAndObligationCollateral";
const WITHDRAW: &str = "withdrawObligationCollateralAndRedeemReserveCollateral";

/// Test-only mirror of the subset of `AccountInfo` the validator needs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RemainingAccountMeta {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

/// One expected account slot read from the fixture plan section.
#[derive(Clone, Debug)]
struct ExpectedAccount {
    name: String,
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

fn fixture() -> Value {
    serde_json::from_str(FIXTURE_JSON).expect("valid Kamino CPI account-plan fixture")
}

/// Canonical ordered account plan for `section`, read from the committed fixture.
fn expected_plan(section: &str) -> Vec<ExpectedAccount> {
    let f = fixture();
    let accounts = f
        .pointer(&format!("/accountPlan/plans/{section}/accounts"))
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("fixture missing plan section {section}"))
        .clone();
    accounts
        .iter()
        .map(|a| ExpectedAccount {
            name: a["name"].as_str().expect("account name").to_string(),
            pubkey: a["pubkey"].as_str().expect("account pubkey").to_string(),
            is_signer: a["isSigner"].as_bool().unwrap_or(false),
            is_writable: a["isWritable"].as_bool().unwrap_or(false),
        })
        .collect()
}

/// Validate a positional slice of remaining accounts against the fixture plan.
/// Fails loudly on count mismatch, wrong order (pubkey at a position), or a
/// signer/writable expectation mismatch.
fn validate_remaining_accounts(
    section: &str,
    provided: &[RemainingAccountMeta],
) -> Result<(), String> {
    let expected = expected_plan(section);
    if provided.len() != expected.len() {
        return Err(format!(
            "{section}: expected {} remaining accounts, got {}",
            expected.len(),
            provided.len()
        ));
    }
    for (i, (exp, got)) in expected.iter().zip(provided.iter()).enumerate() {
        if got.pubkey != exp.pubkey {
            return Err(format!(
                "{section}: account {i} ({}) expected pubkey {} but got {}",
                exp.name, exp.pubkey, got.pubkey
            ));
        }
        if got.is_signer != exp.is_signer {
            return Err(format!(
                "{section}: account {i} ({}) expected is_signer={} but got {}",
                exp.name, exp.is_signer, got.is_signer
            ));
        }
        if got.is_writable != exp.is_writable {
            return Err(format!(
                "{section}: account {i} ({}) expected is_writable={} but got {}",
                exp.name, exp.is_writable, got.is_writable
            ));
        }
    }
    Ok(())
}

/// The happy-path slice a correct dispatcher would supply (straight from fixture).
fn provided_from_fixture(section: &str) -> Vec<RemainingAccountMeta> {
    expected_plan(section)
        .into_iter()
        .map(|e| RemainingAccountMeta {
            pubkey: e.pubkey,
            is_signer: e.is_signer,
            is_writable: e.is_writable,
        })
        .collect()
}

#[test]
fn deposit_remaining_accounts_match_fixture_order() {
    let provided = provided_from_fixture(DEPOSIT);
    validate_remaining_accounts(DEPOSIT, &provided).expect("correct deposit slice must validate");
}

#[test]
fn withdraw_remaining_accounts_match_fixture_order() {
    let provided = provided_from_fixture(WITHDRAW);
    validate_remaining_accounts(WITHDRAW, &provided).expect("correct withdraw slice must validate");
}

#[test]
fn deposit_includes_user_source_liquidity() {
    assert!(
        expected_plan(DEPOSIT)
            .iter()
            .any(|a| a.name == "userSourceLiquidity"),
        "deposit plan must include userSourceLiquidity"
    );
}

#[test]
fn withdraw_includes_user_destination_liquidity() {
    assert!(
        expected_plan(WITHDRAW)
            .iter()
            .any(|a| a.name == "userDestinationLiquidity"),
        "withdraw plan must include userDestinationLiquidity"
    );
}

#[test]
fn missing_account_fails_loudly() {
    let mut provided = provided_from_fixture(DEPOSIT);
    provided.pop();
    let err = validate_remaining_accounts(DEPOSIT, &provided)
        .expect_err("a short slice must be rejected");
    assert!(err.contains("expected"), "count mismatch must be loud: {err}");
}

#[test]
fn wrong_order_fails_loudly() {
    let mut provided = provided_from_fixture(DEPOSIT);
    provided.swap(0, 1);
    let err = validate_remaining_accounts(DEPOSIT, &provided)
        .expect_err("a reordered slice must be rejected");
    assert!(
        err.contains("account 0") && err.contains("owner"),
        "wrong order must be loud: {err}"
    );
}

#[test]
fn signer_expectation_mismatch_fails_loudly() {
    let mut provided = provided_from_fixture(DEPOSIT);
    provided[0].is_signer = false;
    let err = validate_remaining_accounts(DEPOSIT, &provided)
        .expect_err("a signer-flag mismatch must be rejected");
    assert!(err.contains("is_signer"), "signer mismatch must be loud: {err}");
}
