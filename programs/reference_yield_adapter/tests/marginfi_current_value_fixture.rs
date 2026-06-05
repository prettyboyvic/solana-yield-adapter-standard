use std::str::FromStr;

use anchor_lang::prelude::Pubkey;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reference_yield_adapter::{
    marginfi_shares_to_assets, read_marginfi_account_asset_shares,
    read_marginfi_bank_asset_share_value, MARGINFI_PROGRAM_ID, MARGINFI_USDC_BANK, USDC_MINT,
};
use serde_json::Value;

const FIXTURE_JSON: &str =
    include_str!("../../../tests/fixtures/marginfi-current-value-424351480.json");

#[test]
fn marginfi_current_value_fixture_matches_sdk() {
    let fixture: Value = serde_json::from_str(FIXTURE_JSON).expect("valid fixture json");
    assert_eq!(fixture["schema"], "marginfi-current-value-fixture/v1");
    assert_eq!(fixture["slot"].as_u64().unwrap(), 424351480);
    assert_eq!(fixture["marginfiSdk"]["version"], "6.4.2");
    assert_eq!(
        fixture["accounts"]["bank"]["pubkey"].as_str().unwrap(),
        MARGINFI_USDC_BANK.to_string()
    );
    assert_eq!(
        fixture["accounts"]["bank"]["owner"].as_str().unwrap(),
        MARGINFI_PROGRAM_ID.to_string()
    );
    assert_eq!(
        fixture["accounts"]["sampleMarginfiAccount"]["owner"]
            .as_str()
            .unwrap(),
        MARGINFI_PROGRAM_ID.to_string()
    );

    let bank_data = decode_base64(&fixture["accounts"]["bank"]["dataBase64"]);
    let account_data = decode_base64(&fixture["accounts"]["sampleMarginfiAccount"]["dataBase64"]);
    assert_eq!(
        bank_data.len(),
        fixture["accounts"]["bank"]["dataLength"].as_u64().unwrap() as usize
    );
    assert_eq!(
        account_data.len(),
        fixture["accounts"]["sampleMarginfiAccount"]["dataLength"]
            .as_u64()
            .unwrap() as usize
    );

    let expected_authority =
        Pubkey::from_str(fixture["sdkOracle"]["accountAuthority"].as_str().unwrap()).unwrap();
    let asset_share_value = read_marginfi_bank_asset_share_value(&bank_data, USDC_MINT).unwrap();
    let asset_shares =
        read_marginfi_account_asset_shares(&account_data, expected_authority, MARGINFI_USDC_BANK)
            .unwrap();
    assert_eq!(
        asset_share_value,
        fixture["sdkOracle"]["assetShareValueRawI80F48"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap()
    );
    assert_eq!(
        asset_shares,
        fixture["sdkOracle"]["assetSharesRawI80F48"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap()
    );

    let rust_assets = marginfi_shares_to_assets(asset_shares, asset_share_value).unwrap();
    let sdk_expected_assets = fixture["sdkOracle"]["expectedAssets"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let diff_lamports = rust_assets.abs_diff(sdk_expected_assets);

    println!(
        "marginfi_current_value_fixture slot={} asset_shares_raw={} asset_share_value_raw={} rust_assets={} sdk_expected_assets={} diff_lamports={}",
        fixture["slot"].as_u64().unwrap(),
        asset_shares,
        asset_share_value,
        rust_assets,
        sdk_expected_assets,
        diff_lamports
    );

    assert!(
        diff_lamports <= 1,
        "Rust decoder drifted from MarginFi SDK oracle: rust={rust_assets} sdk={sdk_expected_assets} diff={diff_lamports}"
    );
}

fn decode_base64(value: &Value) -> Vec<u8> {
    STANDARD
        .decode(value.as_str().expect("base64 fixture string"))
        .expect("valid base64 account data")
}
