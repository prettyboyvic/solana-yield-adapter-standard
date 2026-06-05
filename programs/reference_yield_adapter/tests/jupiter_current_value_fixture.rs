use anchor_lang::prelude::Pubkey;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reference_yield_adapter::{
    jupiter_current_value_from_data, jupiter_jlp_to_assets, read_jupiter_pool_aum_usd,
    read_spl_mint_supply, JLP_MINT, JUPITER_PERPS_PROGRAM_ID, JUPITER_POOL,
};
use serde_json::Value;

const FIXTURE_JSON: &str =
    include_str!("../../../tests/fixtures/jupiter-current-value-424386975.json");

#[test]
fn jupiter_current_value_fixture_matches_on_chain_idl_oracle() {
    let fixture: Value = serde_json::from_str(FIXTURE_JSON).expect("valid fixture json");
    assert_eq!(fixture["schema"], "jupiter-current-value-fixture/v1");
    assert_eq!(fixture["slot"].as_u64().unwrap(), 424386975);
    assert_eq!(
        fixture["accounts"]["pool"]["pubkey"].as_str().unwrap(),
        JUPITER_POOL.to_string()
    );
    assert_eq!(
        fixture["accounts"]["pool"]["owner"].as_str().unwrap(),
        JUPITER_PERPS_PROGRAM_ID.to_string()
    );
    assert_eq!(
        fixture["accounts"]["jlpMint"]["pubkey"].as_str().unwrap(),
        JLP_MINT.to_string()
    );

    let pool_data = decode_base64(&fixture["accounts"]["pool"]["dataBase64"]);
    let mint_data = decode_base64(&fixture["accounts"]["jlpMint"]["dataBase64"]);
    let aum_usd = read_jupiter_pool_aum_usd(&pool_data).unwrap();
    let jlp_supply = read_spl_mint_supply(&mint_data, 6).unwrap();
    let sample_jlp_amount = fixture["idlOracle"]["sampleJlpAmount"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert_eq!(
        aum_usd,
        fixture["idlOracle"]["aumUsd"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap()
    );
    assert_eq!(
        jlp_supply,
        fixture["idlOracle"]["jlpMintSupply"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
    );

    let expected_assets = fixture["idlOracle"]["expectedAssets"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let rust_assets = jupiter_jlp_to_assets(sample_jlp_amount, aum_usd, jlp_supply).unwrap();
    assert_eq!(rust_assets, expected_assets);

    let owner = Pubkey::new_unique();
    let receipt = build_receipt(owner, sample_jlp_amount);
    assert_eq!(
        jupiter_current_value_from_data(&pool_data, &mint_data, &receipt, owner).unwrap(),
        expected_assets
    );

    println!(
        "jupiter_current_value_fixture slot={} aum_usd={} jlp_supply={} sample_jlp_amount={} expected_assets={}",
        fixture["slot"].as_u64().unwrap(),
        aum_usd,
        jlp_supply,
        sample_jlp_amount,
        expected_assets
    );
}

fn build_receipt(owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; 72];
    data[..32].copy_from_slice(JLP_MINT.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data
}

fn decode_base64(value: &Value) -> Vec<u8> {
    STANDARD
        .decode(value.as_str().expect("base64 fixture string"))
        .expect("valid base64 account data")
}
