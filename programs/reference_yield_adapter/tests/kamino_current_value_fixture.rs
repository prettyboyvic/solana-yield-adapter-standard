use std::str::FromStr;

use anchor_lang::prelude::Pubkey;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reference_yield_adapter::{
    kamino_collateral_to_assets, read_kamino_obligation_collateral,
    read_kamino_reserve_total_supply_and_mint_supply, KAMINO_USDC_OBLIGATION, KAMINO_USDC_RESERVE,
};
use serde_json::Value;

const FIXTURE_JSON: &str =
    include_str!("../../../tests/fixtures/kamino-current-value-424277911.json");

#[test]
fn kamino_current_value_fixture_matches_sdk() {
    let fixture: Value = serde_json::from_str(FIXTURE_JSON).expect("valid fixture json");
    assert_eq!(fixture["schema"], "kamino-current-value-fixture/v1");
    assert_eq!(fixture["slot"].as_u64().unwrap(), 424277911);
    assert_eq!(fixture["klendSdk"]["version"], "3.2.26");
    assert_eq!(
        fixture["accounts"]["reserve"]["pubkey"].as_str().unwrap(),
        KAMINO_USDC_RESERVE.to_string()
    );
    assert_eq!(
        fixture["accounts"]["adapterObligation"]["pubkey"]
            .as_str()
            .unwrap(),
        KAMINO_USDC_OBLIGATION.to_string()
    );
    assert!(
        !fixture["accounts"]["adapterObligation"]["accountFound"]
            .as_bool()
            .unwrap()
    );

    let reserve_data = decode_base64(&fixture["accounts"]["reserve"]["dataBase64"]);
    assert_eq!(
        reserve_data.len(),
        fixture["accounts"]["reserve"]["dataLength"].as_u64().unwrap() as usize
    );
    let obligation_data = decode_base64(&fixture["accounts"]["oracleObligation"]["dataBase64"]);
    assert_eq!(
        obligation_data.len(),
        fixture["accounts"]["oracleObligation"]["dataLength"]
            .as_u64()
            .unwrap() as usize
    );

    let expected_owner =
        Pubkey::from_str(fixture["sdkOracle"]["obligationOwner"].as_str().unwrap()).unwrap();
    let (total_supply, mint_total_supply) =
        read_kamino_reserve_total_supply_and_mint_supply(&reserve_data).unwrap();
    let deposited =
        read_kamino_obligation_collateral(&obligation_data, expected_owner, KAMINO_USDC_RESERVE)
            .unwrap();
    let rust_assets = kamino_collateral_to_assets(deposited, total_supply, mint_total_supply)
        .unwrap();
    let sdk_expected_assets = fixture["sdkOracle"]["expectedAssets"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let diff_lamports = rust_assets.abs_diff(sdk_expected_assets);

    println!(
        "kamino_current_value_fixture slot={} deposited_collateral={} rust_total_supply={} mint_total_supply={} rust_assets={} sdk_expected_assets={} diff_lamports={}",
        fixture["slot"].as_u64().unwrap(),
        deposited,
        total_supply,
        mint_total_supply,
        rust_assets,
        sdk_expected_assets,
        diff_lamports
    );

    assert!(
        diff_lamports <= 1,
        "Rust decoder drifted from klend-sdk oracle: rust={rust_assets} sdk={sdk_expected_assets} diff={diff_lamports}"
    );
}

fn decode_base64(value: &Value) -> Vec<u8> {
    STANDARD
        .decode(value.as_str().expect("base64 fixture string"))
        .expect("valid base64 account data")
}
