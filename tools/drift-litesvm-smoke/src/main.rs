//! Drift Insurance Fund — Gate 2 minimal litesvm smoke test (NOT the adapter).
//!
//! Purpose: prove on the host that the Rust `litesvm` path can support the Drift
//! IF harness, specifically:
//!   1. load (or prepare loading of) a dumped Drift BPF `.so`,
//!   2. load cloned Drift/IF accounts into litesvm,
//!   3. set/warp the Clock sysvar by >= 13 days,
//!   4. execute the smallest possible post-warp transaction path.
//!
//! This file does NOT implement any adapter logic and does NOT claim Drift
//! integration. It only exercises the harness primitives and fails loudly if any
//! primitive does not work. It never fakes success.
//!
//! Two layers:
//!   Layer A (always runs, no artifacts needed): System Program transfer + Clock
//!           warp. Exercises litesvm transaction execution (the System Program is
//!           a builtin, not BPF) and proves unix_timestamp can be advanced past an
//!           unstaking period. Real BPF execution is proven only by Layer B.
//!   Layer B (runs when DRIFT_SO is set): loads the REAL Drift `.so`, injects any
//!           cloned accounts from DRIFT_ACCOUNTS_DIR, warps the clock, then sends a
//!           transaction to the Drift program id and asserts via program logs that
//!           the Drift BPF actually executed (an in-program error still proves
//!           execution; a loader/"program not found" error does not).
//!
//! Env vars:
//!   DRIFT_SO            path to a dumped Drift program .so (enables Layer B)
//!   DRIFT_ACCOUNTS_DIR  dir of `solana account --output json` files to inject (optional)
//!   WARP_DAYS           clock warp in days (default 13)

use std::{env, fs, path::Path, process, str::FromStr};

use base64::Engine;
use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction, system_program,
    sysvar::rent,
    transaction::Transaction,
};

const DRIFT_PROGRAM_ID: &str = "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH";

// --- Real mainnet accounts for the USDC IF (market index 0). Verified addresses;
// the cloned snapshots live in DRIFT_ACCOUNTS_DIR. See docs/drift/layer-c-verification.md.
const DRIFT_STATE: &str = "5zpq7DvB6UdFFvpmBPspGPNfUGoBRRCE2HHg5u3gxcsN";
const DRIFT_SIGNER: &str = "JCNCMFXo5M5qwUPg2Utu1u6YWp3MbygxqBsBeXXJfrw";
const USDC_SPOT_MARKET: &str = "6gMq3mRCKf8aP3ttTyYhuijVZ2LGi14oDsBbkgubfLB3";
const USDC_SPOT_MARKET_VAULT: &str = "GXWqPpjQpdz7KZw9p7f5PX2eGxHAhvpNXiviFkAB8zXg";
const USDC_IF_VAULT: &str = "2CqkQvYxp9Mq4PqLvAQ1eryYxebUh4Liyn5YMDtXsYci";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const USDC_MARKET_INDEX: u16 = 0;

// Verified Anchor instruction discriminators (sha256("global:<name>")[..8]).
// docs/drift/layer-c-verification.md §4. The harness recomputes and asserts these.
const DISC_INIT_USER_STATS: [u8; 8] = [254, 243, 72, 98, 251, 130, 168, 213];
const DISC_INIT_IF_STAKE: [u8; 8] = [187, 179, 243, 70, 248, 90, 92, 147];
const DISC_ADD_IF_STAKE: [u8; 8] = [251, 144, 115, 11, 222, 47, 62, 236];
const DISC_ADMIN_WITHDRAW_IF_VAULT: [u8; 8] = [228, 208, 191, 246, 169, 58, 189, 213];

fn fail(msg: &str) -> ! {
    eprintln!("SMOKE_FAIL: {msg}");
    process::exit(1);
}

fn warp_days() -> i64 {
    env::var("WARP_DAYS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(13)
}

/// Layer A — exercise System Program transfer execution + Clock warp without any
/// external artifacts. (System Program is a builtin, not BPF; real BPF execution
/// is proven only in Layer B.)
fn layer_a(svm: &mut LiteSVM) {
    println!("== Layer A: System Program transfer + clock warp ==");

    let payer = Keypair::new();
    if svm.airdrop(&payer.pubkey(), 10_000_000_000).is_err() {
        fail("airdrop failed");
    }
    let dest = Keypair::new();
    let ix = system_instruction::transfer(&payer.pubkey(), &dest.pubkey(), 1_000_000);
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());
    match svm.send_transaction(tx) {
        Ok(_) => {}
        Err(e) => fail(&format!("system transfer did not execute: {:?}", e.err)),
    }
    let bal = svm.get_balance(&dest.pubkey()).unwrap_or(0);
    if bal != 1_000_000 {
        fail(&format!("unexpected dest balance after transfer: {bal}"));
    }
    println!("[A1] real System-Program transfer executed; dest balance = {bal} lamports");

    let days = warp_days();
    let secs = days * 24 * 60 * 60;
    let c0 = svm.get_sysvar::<Clock>();
    let mut c1 = c0.clone();
    c1.unix_timestamp = c0.unix_timestamp + secs;
    svm.set_sysvar::<Clock>(&c1);
    let c2 = svm.get_sysvar::<Clock>();
    let delta = c2.unix_timestamp - c0.unix_timestamp;
    if delta < secs {
        fail(&format!("clock warp ineffective: delta={delta}s expected>={secs}s"));
    }
    println!(
        "[A2] Clock warped: unix_timestamp {} -> {} (delta {} days)",
        c0.unix_timestamp, c2.unix_timestamp, days
    );
    println!("[A] OK");
}

/// Inject a single `solana account --output json` file into litesvm.
fn inject_account_json(svm: &mut LiteSVM, path: &Path) {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => fail(&format!("read {}: {e}", path.display())),
    };
    let v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => fail(&format!("parse {}: {e}", path.display())),
    };
    let pubkey = v["pubkey"].as_str().unwrap_or_else(|| fail("account json missing pubkey"));
    let acct = &v["account"];
    let lamports = acct["lamports"].as_u64().unwrap_or_else(|| fail("missing lamports"));
    let owner = acct["owner"].as_str().unwrap_or_else(|| fail("missing owner"));
    let executable = acct["executable"].as_bool().unwrap_or(false);
    // data is ["<base64>", "base64"]
    let data_b64 = acct["data"][0].as_str().unwrap_or_else(|| fail("missing data[0]"));
    let data = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .unwrap_or_else(|e| fail(&format!("base64 decode {}: {e}", path.display())));
    let data_len = data.len();

    let account = Account {
        lamports,
        data,
        owner: Pubkey::from_str(owner).unwrap_or_else(|_| fail("bad owner pubkey")),
        executable,
        rent_epoch: 0,
    };
    let pk = Pubkey::from_str(pubkey).unwrap_or_else(|_| fail("bad account pubkey"));
    if svm.set_account(pk, account).is_err() {
        fail(&format!("set_account failed for {pubkey}"));
    }
    // Report the real decoded byte count, not the base64 string length.
    println!("    injected cloned account {pubkey} (owner {owner}, {data_len} bytes)");
}

/// Layer B — load the REAL Drift `.so`, inject cloned accounts, warp, execute.
fn layer_b(svm: &mut LiteSVM, so_path: String) {
    println!("== Layer B: real Drift BPF program ==");
    let pid = Pubkey::from_str(DRIFT_PROGRAM_ID).unwrap();

    if !Path::new(&so_path).exists() {
        fail(&format!("DRIFT_SO does not exist: {so_path}"));
    }
    if svm.add_program_from_file(pid, &so_path).is_err() {
        fail(&format!("add_program_from_file failed for {so_path}"));
    }
    println!("[B1] loaded Drift program {DRIFT_PROGRAM_ID} from {so_path}");

    if let Ok(dir) = env::var("DRIFT_ACCOUNTS_DIR") {
        let mut n = 0;
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "json").unwrap_or(false) {
                    inject_account_json(svm, &p);
                    n += 1;
                }
            }
        }
        println!("[B2] injected {n} cloned account(s) from {dir}");
    } else {
        println!("[B2] DRIFT_ACCOUNTS_DIR not set; skipping account injection");
    }

    // Clock already warped in Layer A; re-affirm here for clarity.
    let clk = svm.get_sysvar::<Clock>();
    println!("[B3] clock unix_timestamp at execution = {}", clk.unix_timestamp);

    // Smallest possible execution against the Drift program: a transaction whose
    // instruction targets the Drift program id with empty data. We do NOT expect
    // success (no valid discriminator/accounts) — we expect the BPF to RUN and
    // reject it. Program logs containing the Drift program-id invoke prove the
    // .so executed (vs a loader error, which would mean it never ran).
    let payer = Keypair::new();
    if svm.airdrop(&payer.pubkey(), 1_000_000_000).is_err() {
        fail("airdrop (layer B) failed");
    }
    let ix = Instruction { program_id: pid, accounts: vec![], data: vec![] };
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[&payer], msg, svm.latest_blockhash());

    let logs: Vec<String> = match svm.send_transaction(tx) {
        Ok(meta) => {
            println!("[B4] Drift tx returned Ok (unexpected but still proves execution)");
            meta.logs
        }
        Err(failed) => {
            println!("[B4] Drift tx failed as expected: {:?}", failed.err);
            failed.meta.logs
        }
    };
    println!("---- program logs ----");
    for l in &logs {
        println!("    {l}");
    }
    println!("----------------------");

    let executed = logs.iter().any(|l| l.contains(DRIFT_PROGRAM_ID) && l.contains("invoke"));
    if !executed {
        fail(
            "Drift BPF did NOT execute (no program-invoke log). This is a loader/feature \
             mismatch, not real execution. Do NOT treat as proven.",
        );
    }
    println!("[B] OK — real Drift BPF executed under litesvm after a {}-day clock warp", warp_days());
}

// ---------------------------------------------------------------------------
// Layer C: real Drift Insurance Fund instruction sequence
// ---------------------------------------------------------------------------

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap_or_else(|_| fail(&format!("bad pubkey const: {s}")))
}

fn anchor_disc(name: &str) -> [u8; 8] {
    let h = hash(format!("global:{name}").as_bytes()).to_bytes();
    let mut d = [0u8; 8];
    d.copy_from_slice(&h[..8]);
    d
}

fn read_u128_le(data: &[u8], off: usize) -> u128 {
    let mut b = [0u8; 16];
    b.copy_from_slice(&data[off..off + 16]);
    u128::from_le_bytes(b)
}

fn read_u64_le(data: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&data[off..off + 8]);
    u64::from_le_bytes(b)
}

/// Build a raw SPL token account (165 bytes): mint, owner, amount, state=Initialized.
fn make_token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut d = vec![0u8; 165];
    d[0..32].copy_from_slice(mint.as_ref());
    d[32..64].copy_from_slice(owner.as_ref());
    d[64..72].copy_from_slice(&amount.to_le_bytes());
    d[108] = 1; // AccountState::Initialized
    d
}

/// Read an SPL token account amount (offset 64..72) from litesvm, or None if missing.
fn token_amount(svm: &LiteSVM, addr: &Pubkey) -> Option<u64> {
    svm.get_account(addr).map(|a| read_u64_le(&a.data, 64))
}

fn send_named(
    svm: &mut LiteSVM,
    label: &str,
    ix: Instruction,
    signers: &[&Keypair],
    payer: &Pubkey,
) -> bool {
    let msg = Message::new(&[ix], Some(payer));
    let tx = Transaction::new(signers, msg, svm.latest_blockhash());
    let (ok, logs, errline) = match svm.send_transaction(tx) {
        Ok(meta) => (true, meta.logs, String::new()),
        Err(failed) => (false, failed.meta.logs, format!("{:?}", failed.err)),
    };
    println!("---- {label} logs ----");
    for l in &logs {
        println!("    {l}");
    }
    if !ok {
        println!("    ERROR: {errline}");
    }
    println!("----------------------");
    let executed = logs.iter().any(|l| l.contains(DRIFT_PROGRAM_ID) && l.contains("invoke"));
    if !executed {
        fail(&format!("{label}: Drift program did not execute (no invoke log)"));
    }
    ok
}

/// Preflight: does the loaded program dispatch this discriminator at all?
/// Sends a discriminator-only instruction (no accounts). If the program recognizes
/// the discriminator it enters the handler and fails later (deserialize / missing
/// accounts); if it does NOT, Anchor returns `InstructionFallbackNotFound` (101).
/// Returns true iff the instruction is present in the loaded binary.
fn program_dispatches(svm: &mut LiteSVM, drift: &Pubkey, payer: &Keypair, disc: &[u8; 8]) -> bool {
    let ix = Instruction { program_id: *drift, accounts: vec![], data: disc.to_vec() };
    let msg = Message::new(&[ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, svm.latest_blockhash());
    let logs = match svm.send_transaction(tx) {
        Ok(meta) => meta.logs,
        Err(failed) => failed.meta.logs,
    };
    !logs.iter().any(|l| l.contains("InstructionFallbackNotFound"))
}

fn layer_c_dispatch_preflight(svm: &mut LiteSVM) {
    println!("\n== Layer C preflight: Drift instruction dispatch ==");

    let drift = pk(DRIFT_PROGRAM_ID);
    let payer = Keypair::new();
    if svm.airdrop(&payer.pubkey(), 1_000_000_000).is_err() {
        fail("Layer C preflight airdrop failed");
    }

    assert_eq!(
        anchor_disc("admin_withdraw_from_insurance_fund_vault"),
        DISC_ADMIN_WITHDRAW_IF_VAULT,
        "known-present admin withdraw disc"
    );
    if !program_dispatches(svm, &drift, &payer, &DISC_ADMIN_WITHDRAW_IF_VAULT) {
        fail(
            "Layer C preflight control failed: known-present \
             admin_withdraw_from_insurance_fund_vault did not dispatch",
        );
    }
    println!(
        "[C-PREFLIGHT] known-present `admin_withdraw_from_insurance_fund_vault` dispatches"
    );

    let mut missing_if_handler = false;
    for (name, disc) in [
        ("initialize_insurance_fund_stake", &DISC_INIT_IF_STAKE),
        ("add_insurance_fund_stake", &DISC_ADD_IF_STAKE),
    ] {
        if program_dispatches(svm, &drift, &payer, disc) {
            println!("[C-PREFLIGHT] required `{name}` dispatches");
        } else {
            println!(
                "[C-PREFLIGHT] required `{name}` returned Anchor 101 \
                 (`InstructionFallbackNotFound`)"
            );
            missing_if_handler = true;
        }
    }

    if missing_if_handler {
        eprintln!("LAYER_C_BLOCKED: dumped Drift binary lacks IF-stake instruction handlers");
        process::exit(1);
    }
    println!("[C-PREFLIGHT] OK — required IF-stake instruction handlers dispatch");
}

fn layer_c(svm: &mut LiteSVM) {
    println!("\n== Layer C: real Drift Insurance Fund instruction sequence ==");

    // 1. Recompute + assert discriminators match the verified bytes (verified against
    // @drift-labs/sdk BorshInstructionCoder for program dRiftyHA — see
    // docs/drift/layer-c-verification.md).
    assert_eq!(anchor_disc("initialize_user_stats"), DISC_INIT_USER_STATS, "init_user_stats disc");
    assert_eq!(anchor_disc("initialize_insurance_fund_stake"), DISC_INIT_IF_STAKE, "init_if_stake disc");
    assert_eq!(anchor_disc("add_insurance_fund_stake"), DISC_ADD_IF_STAKE, "add_if_stake disc");
    println!("[C0] discriminators recomputed in-harness and match SDK-verified bytes");

    let drift = pk(DRIFT_PROGRAM_ID);
    let state = pk(DRIFT_STATE);
    let drift_signer = pk(DRIFT_SIGNER);
    let spot_market = pk(USDC_SPOT_MARKET);
    let spot_market_vault = pk(USDC_SPOT_MARKET_VAULT);
    let if_vault = pk(USDC_IF_VAULT);
    let usdc_mint = pk(USDC_MINT);
    let token_program = pk(TOKEN_PROGRAM);

    // 2. Fresh local authority (also fee payer); airdrop SOL.
    let authority = Keypair::new();
    let auth_pk = authority.pubkey();
    if svm.airdrop(&auth_pk, 5_000_000_000).is_err() {
        fail("layer C airdrop failed");
    }

    // 3. Derive runtime PDAs from official Drift seeds.
    let mi_le = USDC_MARKET_INDEX.to_le_bytes();
    let (user_stats, _) = Pubkey::find_program_address(&[b"user_stats", auth_pk.as_ref()], &drift);
    let (if_stake, _) =
        Pubkey::find_program_address(&[b"insurance_fund_stake", auth_pk.as_ref(), &mi_le], &drift);
    println!("[C1] authority      = {auth_pk}");
    println!("     user_stats PDA = {user_stats}");
    println!("     if_stake PDA   = {if_stake}");

    // 4. Stage a funded USDC token account owned by authority.
    let user_usdc_kp = Keypair::new();
    let user_usdc = user_usdc_kp.pubkey();
    let funded: u64 = 10_000_000; // 10 USDC
    let rent_lamports = svm.minimum_balance_for_rent_exemption(165);
    let ta = Account {
        lamports: rent_lamports,
        data: make_token_account(&usdc_mint, &auth_pk, funded),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    if svm.set_account(user_usdc, ta).is_err() {
        fail("failed to stage user USDC token account");
    }
    println!("[C2] user USDC token account {user_usdc} funded with {funded} (10 USDC)");

    // 4b. Realign the clock to a realistic timestamp. Layers A/B warped unix_timestamp
    // to ~0+Nd (epoch ~1.2M), but the cloned mainnet accounts hold real 2026
    // timestamps (~1.75e9). add_insurance_fund_stake does revenue-settle time math
    // (`now - last_revenue_settle_ts`); a `now` far in the past underflows -> custom
    // error. Set now = SpotMarket.insurance_fund.last_revenue_settle_ts (offset
    // 392..400, i64 LE) so time deltas are non-negative and no settle is forced.
    // (The 13/14-day cooldown warp is only relevant to the later remove step.)
    let sm_data = svm
        .get_account(&spot_market)
        .unwrap_or_else(|| fail("spot_market not loaded"))
        .data;
    let last_rev_settle_ts = i64::from_le_bytes(sm_data[392..400].try_into().unwrap());
    let mut clk = svm.get_sysvar::<Clock>();
    clk.unix_timestamp = last_rev_settle_ts.max(clk.unix_timestamp);
    svm.set_sysvar::<Clock>(&clk);
    println!("[C2b] clock realigned to unix_timestamp = {} (SpotMarket.last_revenue_settle_ts)", clk.unix_timestamp);

    // 5. Before-state.
    let if_vault_before = token_amount(svm, &if_vault).unwrap_or(0);
    let user_before = token_amount(svm, &user_usdc).unwrap_or(0);
    let total_shares_before = svm
        .get_account(&spot_market)
        .map(|a| read_u128_le(&a.data, 336))
        .unwrap_or(0);
    println!("[C3] BEFORE  user_usdc={user_before}  if_vault={if_vault_before}  total_shares={total_shares_before}");

    let stake_amount: u64 = 1_000_000; // 1 USDC

    // 6a. initialize_user_stats — [userStats(w), state(w), authority(ro), payer(w,signer), rent, system]
    let ix_ius = Instruction {
        program_id: drift,
        accounts: vec![
            AccountMeta::new(user_stats, false),
            AccountMeta::new(state, false),
            AccountMeta::new_readonly(auth_pk, false),
            AccountMeta::new(auth_pk, true),
            AccountMeta::new_readonly(rent::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DISC_INIT_USER_STATS.to_vec(),
    };
    send_named(svm, "initialize_user_stats", ix_ius, &[&authority], &auth_pk);

    // 6b. initialize_insurance_fund_stake(market_index=0)
    //     [spotMarket(ro), ifStake(w), userStats(w), state(ro), authority(ro,signer), payer(w,signer), rent, system]
    let mut data_iifs = DISC_INIT_IF_STAKE.to_vec();
    data_iifs.extend_from_slice(&USDC_MARKET_INDEX.to_le_bytes());
    let ix_iifs = Instruction {
        program_id: drift,
        accounts: vec![
            AccountMeta::new_readonly(spot_market, false),
            AccountMeta::new(if_stake, false),
            AccountMeta::new(user_stats, false),
            AccountMeta::new_readonly(state, false),
            AccountMeta::new_readonly(auth_pk, true),
            AccountMeta::new(auth_pk, true),
            AccountMeta::new_readonly(rent::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: data_iifs,
    };
    send_named(svm, "initialize_insurance_fund_stake", ix_iifs, &[&authority], &auth_pk);

    // 6c. add_insurance_fund_stake(market_index=0, amount)
    //     [state(ro), spotMarket(w), ifStake(w), userStats(w), authority(ro,signer),
    //      spotMarketVault(w), ifVault(w), driftSigner(ro), userTokenAccount(w), tokenProgram(ro)]
    let mut data_add = DISC_ADD_IF_STAKE.to_vec();
    data_add.extend_from_slice(&USDC_MARKET_INDEX.to_le_bytes());
    data_add.extend_from_slice(&stake_amount.to_le_bytes());
    let ix_add = Instruction {
        program_id: drift,
        accounts: vec![
            AccountMeta::new_readonly(state, false),
            AccountMeta::new(spot_market, false),
            AccountMeta::new(if_stake, false),
            AccountMeta::new(user_stats, false),
            AccountMeta::new_readonly(auth_pk, true),
            AccountMeta::new(spot_market_vault, false),
            AccountMeta::new(if_vault, false),
            AccountMeta::new_readonly(drift_signer, false),
            AccountMeta::new(user_usdc, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: data_add,
    };
    let add_ok = send_named(svm, "add_insurance_fund_stake", ix_add, &[&authority], &auth_pk);

    // 7. After-state + assertions.
    let if_shares = svm
        .get_account(&if_stake)
        .map(|a| read_u128_le(&a.data, 40))
        .unwrap_or(0);
    let if_vault_after = token_amount(svm, &if_vault).unwrap_or(0);
    let user_after = token_amount(svm, &user_usdc).unwrap_or(0);
    let total_shares_after = svm
        .get_account(&spot_market)
        .map(|a| read_u128_le(&a.data, 336))
        .unwrap_or(0);
    println!("[C4] AFTER   user_usdc={user_after}  if_vault={if_vault_after}  total_shares={total_shares_after}");
    println!("[C5] insuranceFundStake.if_shares = {if_shares}");

    if !add_ok {
        fail("add_insurance_fund_stake did not succeed (see ERROR above); not faking success");
    }
    if if_shares == 0 {
        fail("add_insurance_fund_stake returned Ok but if_shares == 0; investigate, do not claim success");
    }
    println!("[C] OK — real add_insurance_fund_stake succeeded; if_shares={if_shares} (> 0)");
}

fn main() {
    println!("Drift litesvm smoke test (Gate 2). This does NOT prove full Drift integration.\n");

    // Layer A needs only the default builtins. For Layer B, the cloned mainnet
    // Drift program may expect mainnet feature gates to be active; if Layer B
    // prints no program-invoke log, align the feature set here. The exact builder
    // method depends on the litesvm version:
    //   - some versions: LiteSVM::new().with_mainnet_features().with_feature_accounts()
    //   - litesvm 0.6.x: use .with_feature_set(FeatureSet::all_enabled()) instead
    //     (import solana_sdk::feature_set::FeatureSet).
    // Left at defaults so Layer A always compiles/runs; enable only if Layer B
    // requires it. Do NOT block Layer A on this.
    let mut svm = LiteSVM::new();

    layer_a(&mut svm);

    match env::var("DRIFT_SO") {
        Ok(so) if !so.is_empty() => {
            layer_b(&mut svm, so);
            layer_c_dispatch_preflight(&mut svm);
            layer_c(&mut svm);
            println!(
                "\nSMOKE_OK: real Drift BPF execution + clock warp + real add_insurance_fund_stake proven on host."
            );
        }
        _ => {
            println!(
                "\nSMOKE_OK (Layer A only): native execution + clock warp proven. \
                 Set DRIFT_SO to also run Layer B against the real Drift program."
            );
        }
    }
}
