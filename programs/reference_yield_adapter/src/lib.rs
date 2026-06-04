#![allow(deprecated, unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

declare_id!("BCvRj9JakpU1mpo67yt7WjknSAcTqAJMWCSyurcRhBb1");

pub const ADAPTER_SEED: &[u8] = b"adapter";
pub const POSITION_SEED: &[u8] = b"position";
pub const MAX_METADATA_URI_LEN: usize = 96;
pub const BPS_DENOMINATOR: u128 = 10_000;
pub const SLOT_RATE_DENOMINATOR: u128 = 1_000_000;

#[program]
pub mod reference_yield_adapter {
    use super::*;

    pub fn initialize_adapter(
        ctx: Context<InitializeAdapter>,
        adapter_id: [u8; 32],
        config: AdapterConfigInput,
    ) -> Result<()> {
        require!(
            config.metadata_uri.len() <= MAX_METADATA_URI_LEN,
            AdapterError::MetadataUriTooLong
        );
        require!(
            ProtocolKind::try_from(config.protocol).is_ok(),
            AdapterError::InvalidProtocol
        );

        let state = &mut ctx.accounts.state;
        state.adapter_id = adapter_id;
        state.protocol = config.protocol;
        state.authority = ctx.accounts.authority.key();
        state.underlying_mint = config.underlying_mint;
        state.receipt_mint = config.receipt_mint;
        state.protocol_market = config.protocol_market;
        state.value_oracle = config.value_oracle;
        state.total_assets = 0;
        state.total_shares = 0;
        state.last_update_slot = Clock::get()?.slot;
        let (_, state_bump) =
            Pubkey::find_program_address(&[ADAPTER_SEED, adapter_id.as_ref()], ctx.program_id);
        state.bump = state_bump;
        state.paused = false;
        state.metadata_uri = config.metadata_uri;

        emit!(AdapterInitialized {
            adapter_id,
            protocol: state.protocol,
            authority: state.authority,
            underlying_mint: state.underlying_mint,
            receipt_mint: state.receipt_mint,
        });

        Ok(())
    }

    pub fn set_paused(ctx: Context<AdapterAdmin>, adapter_id: [u8; 32], paused: bool) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.state.authority,
            AdapterError::Unauthorized
        );
        require!(
            ctx.accounts.state.adapter_id == adapter_id,
            AdapterError::AdapterMismatch
        );

        ctx.accounts.state.paused = paused;
        emit!(AdapterPaused { adapter_id, paused });
        Ok(())
    }

    /// SIMULATED REFERENCE ONLY: virtual-yield accounting, not bounty-grade
    /// protocol CPI. Use the `*_cpi` routes for real protocol integration.
    pub fn deposit(
        ctx: Context<AdapterRoute>,
        adapter_id: [u8; 32],
        amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        require!(amount > 0, AdapterError::InvalidAmount);
        assert_route(&ctx.accounts.state, &ctx.accounts.position, &ctx.accounts.user, adapter_id)?;
        refresh_virtual_yield(&mut ctx.accounts.state)?;

        let shares_out = quote_deposit_shares(&ctx.accounts.state, amount)?;
        require!(shares_out >= min_shares_out, AdapterError::SlippageExceeded);

        let (_, position_bump) = Pubkey::find_program_address(
            &[
                POSITION_SEED,
                adapter_id.as_ref(),
                ctx.accounts.user.key().as_ref(),
            ],
            ctx.program_id,
        );
        initialize_position_if_needed(
            &mut ctx.accounts.position,
            ctx.accounts.user.key(),
            adapter_id,
            position_bump,
        );

        ctx.accounts.position.shares = ctx
            .accounts
            .position
            .shares
            .checked_add(shares_out)
            .ok_or(AdapterError::MathOverflow)?;
        ctx.accounts.position.principal_assets = ctx
            .accounts
            .position
            .principal_assets
            .checked_add(amount)
            .ok_or(AdapterError::MathOverflow)?;
        ctx.accounts.state.total_assets = ctx
            .accounts
            .state
            .total_assets
            .checked_add(amount)
            .ok_or(AdapterError::MathOverflow)?;
        ctx.accounts.state.total_shares = ctx
            .accounts
            .state
            .total_shares
            .checked_add(shares_out)
            .ok_or(AdapterError::MathOverflow)?;

        update_position_value(&ctx.accounts.state, &mut ctx.accounts.position)?;

        emit!(AdapterDeposit {
            adapter_id,
            user: ctx.accounts.user.key(),
            amount,
            shares_out,
            position_value: ctx.accounts.position.last_value_assets,
        });

        Ok(())
    }

    pub fn withdraw(
        ctx: Context<AdapterRoute>,
        adapter_id: [u8; 32],
        shares: u64,
        min_assets_out: u64,
    ) -> Result<()> {
        require!(shares > 0, AdapterError::InvalidAmount);
        assert_route(&ctx.accounts.state, &ctx.accounts.position, &ctx.accounts.user, adapter_id)?;
        refresh_virtual_yield(&mut ctx.accounts.state)?;

        require!(
            ctx.accounts.position.shares >= shares,
            AdapterError::InsufficientShares
        );
        let assets_out = quote_withdraw_assets(&ctx.accounts.state, shares)?;
        require!(assets_out >= min_assets_out, AdapterError::SlippageExceeded);

        ctx.accounts.position.shares = ctx
            .accounts
            .position
            .shares
            .checked_sub(shares)
            .ok_or(AdapterError::MathOverflow)?;
        ctx.accounts.state.total_shares = ctx
            .accounts
            .state
            .total_shares
            .checked_sub(shares)
            .ok_or(AdapterError::MathOverflow)?;
        ctx.accounts.state.total_assets = ctx
            .accounts
            .state
            .total_assets
            .checked_sub(assets_out)
            .ok_or(AdapterError::MathOverflow)?;

        update_position_value(&ctx.accounts.state, &mut ctx.accounts.position)?;

        emit!(AdapterWithdraw {
            adapter_id,
            user: ctx.accounts.user.key(),
            shares,
            assets_out,
            position_value: ctx.accounts.position.last_value_assets,
        });

        Ok(())
    }

    pub fn current_value(ctx: Context<AdapterRoute>, adapter_id: [u8; 32]) -> Result<()> {
        assert_route(&ctx.accounts.state, &ctx.accounts.position, &ctx.accounts.user, adapter_id)?;
        refresh_virtual_yield(&mut ctx.accounts.state)?;
        update_position_value(&ctx.accounts.state, &mut ctx.accounts.position)?;

        emit!(AdapterValue {
            adapter_id,
            user: ctx.accounts.user.key(),
            shares: ctx.accounts.position.shares,
            value_assets: ctx.accounts.position.last_value_assets,
        });

        Ok(())
    }

    /// Real-CPI deposit route. Carries the SPL token plumbing plus protocol
    /// accounts (via `remaining_accounts`) needed to move real funds. The
    /// protocol-specific CPI body is NOT implemented yet, so this route fails
    /// LOUDLY and never falls back to the simulated/reference yield path.
    pub fn deposit_cpi<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
        adapter_id: [u8; 32],
        amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        let _ = (amount, min_shares_out);
        guard_cpi_route(&ctx, adapter_id)?;
        reject_unimplemented_cpi(ctx.remaining_accounts.len())
    }

    /// Real-CPI withdraw route. See `deposit_cpi`: fails loudly, no simulation.
    pub fn withdraw_cpi<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
        adapter_id: [u8; 32],
        shares: u64,
        min_assets_out: u64,
    ) -> Result<()> {
        let _ = (shares, min_assets_out);
        guard_cpi_route(&ctx, adapter_id)?;
        reject_unimplemented_cpi(ctx.remaining_accounts.len())
    }

    /// Real-CPI value refresh route. See `deposit_cpi`: fails loudly, no simulation.
    pub fn current_value_cpi<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
        adapter_id: [u8; 32],
    ) -> Result<()> {
        guard_cpi_route(&ctx, adapter_id)?;
        reject_unimplemented_cpi(ctx.remaining_accounts.len())
    }
}

/// Routing decision for the real-CPI path. Intentionally has NO variant that
/// represents a successful or simulated outcome, so the real-CPI route cannot
/// silently fall back to the simulated reference yield.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RouteResolution {
    MissingCpiAccounts,
    NotImplemented,
}

/// Pure decision function (unit-testable without a validator).
pub fn resolve_cpi_route(protocol_account_count: usize) -> RouteResolution {
    if protocol_account_count == 0 {
        RouteResolution::MissingCpiAccounts
    } else {
        RouteResolution::NotImplemented
    }
}

/// Map the routing decision onto a loud, explicit program error. Never `Ok`.
fn reject_unimplemented_cpi(protocol_account_count: usize) -> Result<()> {
    match resolve_cpi_route(protocol_account_count) {
        RouteResolution::MissingCpiAccounts => err!(AdapterError::MissingCpiAccounts),
        RouteResolution::NotImplemented => err!(AdapterError::CpiNotImplemented),
    }
}

fn guard_cpi_route<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
    adapter_id: [u8; 32],
) -> Result<()> {
    let state = &ctx.accounts.state;
    require!(state.adapter_id == adapter_id, AdapterError::AdapterMismatch);
    require!(!state.paused, AdapterError::Paused);
    require_keys_eq!(
        ctx.accounts.user_underlying.owner,
        ctx.accounts.user.key(),
        AdapterError::Unauthorized
    );
    Ok(())
}

/// Expected (is_signer, is_writable) metadata for one account slot of a Kamino
/// klend instruction. Sourced from the klend IDL and tied to the canonical
/// fixture (`packages/sdk/fixtures/kamino-cpi-account-plan.json`) by the unit
/// tests below. The on-chain program intentionally stores only these compact
/// flag specs as the routing gate's expectation; it never embeds the fixture JSON.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AccountLayoutSpec {
    pub is_signer: bool,
    pub is_writable: bool,
}

const fn spec(is_signer: bool, is_writable: bool) -> AccountLayoutSpec {
    AccountLayoutSpec { is_signer, is_writable }
}

/// klend `initUserMetadata` account layout (6 accounts).
pub const KAMINO_INIT_USER_METADATA_LAYOUT: [AccountLayoutSpec; 6] = [
    spec(true, false),  // owner
    spec(true, true),   // feePayer
    spec(false, true),  // userMetadata
    spec(false, false), // referrerUserMetadata (optional-none)
    spec(false, false), // rent
    spec(false, false), // systemProgram
];

/// klend `initObligation` account layout (9 accounts).
pub const KAMINO_INIT_OBLIGATION_LAYOUT: [AccountLayoutSpec; 9] = [
    spec(true, false),  // obligationOwner
    spec(true, true),   // feePayer
    spec(false, true),  // obligation
    spec(false, false), // lendingMarket
    spec(false, false), // seed1Account
    spec(false, false), // seed2Account
    spec(false, false), // ownerUserMetadata
    spec(false, false), // rent
    spec(false, false), // systemProgram
];

/// Validate a provided account layout against an expected one. Returns false on a
/// count mismatch or any per-slot signer/writable mismatch (callers fail loudly).
pub fn account_layout_matches(
    expected: &[AccountLayoutSpec],
    provided: &[AccountLayoutSpec],
) -> bool {
    expected.len() == provided.len() && expected.iter().zip(provided).all(|(e, g)| e == g)
}

/// Derive the adapter `state` PDA + bump. This PDA is the Kamino obligation /
/// vault owner; CPI signer seeds are `[ADAPTER_SEED, adapter_id, &[bump]]`.
pub fn adapter_state_pda(adapter_id: &[u8; 32], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ADAPTER_SEED, adapter_id.as_ref()], program_id)
}

fn assert_route(
    state: &AdapterState,
    position: &Position,
    user: &Signer,
    adapter_id: [u8; 32],
) -> Result<()> {
    require!(state.adapter_id == adapter_id, AdapterError::AdapterMismatch);
    require!(!state.paused, AdapterError::Paused);
    if position.owner != Pubkey::default() {
        require_keys_eq!(position.owner, user.key(), AdapterError::Unauthorized);
        require!(
            position.adapter_id == adapter_id,
            AdapterError::AdapterMismatch
        );
    }
    Ok(())
}

fn initialize_position_if_needed(
    position: &mut Position,
    owner: Pubkey,
    adapter_id: [u8; 32],
    bump: u8,
) {
    if position.owner == Pubkey::default() {
        position.owner = owner;
        position.adapter_id = adapter_id;
        position.shares = 0;
        position.principal_assets = 0;
        position.last_value_assets = 0;
        position.bump = bump;
    }
}

fn refresh_virtual_yield(state: &mut AdapterState) -> Result<()> {
    let slot = Clock::get()?.slot;
    let elapsed = slot.saturating_sub(state.last_update_slot);
    if elapsed == 0 || state.total_assets == 0 {
        state.last_update_slot = slot;
        return Ok(());
    }

    let slot_rate_bps = ProtocolKind::try_from(state.protocol)?.slot_rate_bps() as u128;
    let gain = (state.total_assets as u128)
        .checked_mul(elapsed as u128)
        .and_then(|value| value.checked_mul(slot_rate_bps))
        .and_then(|value| value.checked_div(BPS_DENOMINATOR * SLOT_RATE_DENOMINATOR))
        .ok_or(AdapterError::MathOverflow)?;

    state.total_assets = (state.total_assets as u128)
        .checked_add(gain)
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| error!(AdapterError::MathOverflow))?;
    state.last_update_slot = slot;
    Ok(())
}

fn quote_deposit_shares(state: &AdapterState, amount: u64) -> Result<u64> {
    if state.total_assets == 0 || state.total_shares == 0 {
        return Ok(amount);
    }

    (amount as u128)
        .checked_mul(state.total_shares as u128)
        .and_then(|value| value.checked_div(state.total_assets as u128))
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| error!(AdapterError::MathOverflow))
}

fn quote_withdraw_assets(state: &AdapterState, shares: u64) -> Result<u64> {
    require!(state.total_shares > 0, AdapterError::InsufficientShares);
    (shares as u128)
        .checked_mul(state.total_assets as u128)
        .and_then(|value| value.checked_div(state.total_shares as u128))
        .ok_or(AdapterError::MathOverflow)?
        .try_into()
        .map_err(|_| error!(AdapterError::MathOverflow))
}

fn update_position_value(state: &AdapterState, position: &mut Position) -> Result<()> {
    position.last_value_assets = if position.shares == 0 {
        0
    } else {
        quote_withdraw_assets(state, position.shares)?
    };
    Ok(())
}

#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct InitializeAdapter<'info> {
    #[cfg_attr(
        not(feature = "idl-build"),
        account(
            init,
            payer = payer,
            space = AdapterState::SPACE,
            seeds = [ADAPTER_SEED, adapter_id.as_ref()],
            bump
        )
    )]
    #[cfg_attr(
        feature = "idl-build",
        account(
            init,
            payer = payer,
            space = AdapterState::SPACE
        )
    )]
    pub state: Account<'info, AdapterState>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct AdapterAdmin<'info> {
    #[cfg_attr(
        not(feature = "idl-build"),
        account(mut, seeds = [ADAPTER_SEED, adapter_id.as_ref()], bump = state.bump)
    )]
    #[cfg_attr(feature = "idl-build", account(mut))]
    pub state: Account<'info, AdapterState>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct AdapterRoute<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[cfg_attr(
        not(feature = "idl-build"),
        account(mut, seeds = [ADAPTER_SEED, adapter_id.as_ref()], bump = state.bump)
    )]
    #[cfg_attr(feature = "idl-build", account(mut))]
    pub state: Account<'info, AdapterState>,
    #[cfg_attr(
        not(feature = "idl-build"),
        account(
            init_if_needed,
            payer = user,
            space = Position::SPACE,
            seeds = [POSITION_SEED, adapter_id.as_ref(), user.key().as_ref()],
            bump
        )
    )]
    #[cfg_attr(feature = "idl-build", account(mut))]
    pub position: Account<'info, Position>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct AdapterCpiRoute<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[cfg_attr(
        not(feature = "idl-build"),
        account(mut, seeds = [ADAPTER_SEED, adapter_id.as_ref()], bump = state.bump)
    )]
    #[cfg_attr(feature = "idl-build", account(mut))]
    pub state: Account<'info, AdapterState>,
    #[cfg_attr(
        not(feature = "idl-build"),
        account(
            init_if_needed,
            payer = user,
            space = Position::SPACE,
            seeds = [POSITION_SEED, adapter_id.as_ref(), user.key().as_ref()],
            bump
        )
    )]
    #[cfg_attr(feature = "idl-build", account(mut))]
    pub position: Account<'info, Position>,
    /// User's underlying (e.g. USDC) token account funding the deposit/withdraw.
    #[account(mut, constraint = user_underlying.owner == user.key() @ AdapterError::Unauthorized)]
    pub user_underlying: Account<'info, TokenAccount>,
    /// Adapter-side vault / receipt-holding token account.
    #[account(mut)]
    pub adapter_underlying: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    // remaining_accounts: protocol-specific CPI accounts (reserve/bank/pool/oracle/...).
}

#[account]
pub struct AdapterState {
    pub adapter_id: [u8; 32],
    pub protocol: u8,
    pub authority: Pubkey,
    pub underlying_mint: Pubkey,
    pub receipt_mint: Pubkey,
    pub protocol_market: Pubkey,
    pub value_oracle: Pubkey,
    pub total_assets: u64,
    pub total_shares: u64,
    pub last_update_slot: u64,
    pub bump: u8,
    pub paused: bool,
    pub metadata_uri: String,
}

impl AdapterState {
    pub const SPACE: usize = 8 + 32 + 1 + 32 + 32 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + (4 + MAX_METADATA_URI_LEN);
}

#[account]
pub struct Position {
    pub owner: Pubkey,
    pub adapter_id: [u8; 32],
    pub shares: u64,
    pub principal_assets: u64,
    pub last_value_assets: u64,
    pub bump: u8,
}

impl Position {
    pub const SPACE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdapterConfigInput {
    pub protocol: u8,
    pub underlying_mint: Pubkey,
    pub receipt_mint: Pubkey,
    pub protocol_market: Pubkey,
    pub value_oracle: Pubkey,
    pub metadata_uri: String,
}

#[derive(Clone, Copy)]
pub enum ProtocolKind {
    KaminoUsdc = 1,
    MarginfiUsdc = 2,
    JupiterLp = 3,
    MapleSyrup = 4,
    DriftInsuranceFund = 5,
}

impl ProtocolKind {
    pub fn slot_rate_bps(self) -> u64 {
        match self {
            ProtocolKind::KaminoUsdc => 18,
            ProtocolKind::MarginfiUsdc => 16,
            ProtocolKind::JupiterLp => 28,
            ProtocolKind::MapleSyrup => 22,
            ProtocolKind::DriftInsuranceFund => 30,
        }
    }
}

impl TryFrom<u8> for ProtocolKind {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(ProtocolKind::KaminoUsdc),
            2 => Ok(ProtocolKind::MarginfiUsdc),
            3 => Ok(ProtocolKind::JupiterLp),
            4 => Ok(ProtocolKind::MapleSyrup),
            5 => Ok(ProtocolKind::DriftInsuranceFund),
            _ => err!(AdapterError::InvalidProtocol),
        }
    }
}

#[event]
pub struct AdapterInitialized {
    pub adapter_id: [u8; 32],
    pub protocol: u8,
    pub authority: Pubkey,
    pub underlying_mint: Pubkey,
    pub receipt_mint: Pubkey,
}

#[event]
pub struct AdapterPaused {
    pub adapter_id: [u8; 32],
    pub paused: bool,
}

#[event]
pub struct AdapterDeposit {
    pub adapter_id: [u8; 32],
    pub user: Pubkey,
    pub amount: u64,
    pub shares_out: u64,
    pub position_value: u64,
}

#[event]
pub struct AdapterWithdraw {
    pub adapter_id: [u8; 32],
    pub user: Pubkey,
    pub shares: u64,
    pub assets_out: u64,
    pub position_value: u64,
}

#[event]
pub struct AdapterValue {
    pub adapter_id: [u8; 32],
    pub user: Pubkey,
    pub shares: u64,
    pub value_assets: u64,
}

#[error_code]
pub enum AdapterError {
    #[msg("metadata URI exceeds the configured maximum length")]
    MetadataUriTooLong,
    #[msg("protocol kind is invalid")]
    InvalidProtocol,
    #[msg("caller is not authorized")]
    Unauthorized,
    #[msg("adapter id mismatch")]
    AdapterMismatch,
    #[msg("adapter is paused")]
    Paused,
    #[msg("amount must be greater than zero")]
    InvalidAmount,
    #[msg("slippage constraint was not satisfied")]
    SlippageExceeded,
    #[msg("position does not have enough shares")]
    InsufficientShares,
    #[msg("math overflow")]
    MathOverflow,
    #[msg("real-CPI route is missing required protocol accounts")]
    MissingCpiAccounts,
    #[msg("protocol CPI is not implemented yet; simulated fallback is intentionally disabled")]
    CpiNotImplemented,
}

#[cfg(test)]
mod cpi_route_tests {
    use super::*;

    #[test]
    fn missing_protocol_accounts_fails_loudly() {
        assert_eq!(resolve_cpi_route(0), RouteResolution::MissingCpiAccounts);
    }

    #[test]
    fn present_accounts_route_is_not_implemented_not_simulated() {
        // Must be the not-implemented decision, never a success/simulated result.
        assert_eq!(resolve_cpi_route(3), RouteResolution::NotImplemented);
    }

    // ---- Kamino remaining-account validation gate ----

    const FIXTURE_JSON: &str =
        include_str!("../../../packages/sdk/fixtures/kamino-cpi-account-plan.json");

    fn fixture_layout(section: &str) -> Vec<AccountLayoutSpec> {
        let f: serde_json::Value =
            serde_json::from_str(FIXTURE_JSON).expect("valid Kamino CPI account-plan fixture");
        f.pointer(&format!("/accountPlan/plans/{section}/accounts"))
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("fixture missing plan section {section}"))
            .iter()
            .map(|a| AccountLayoutSpec {
                is_signer: a["isSigner"].as_bool().unwrap_or(false),
                is_writable: a["isWritable"].as_bool().unwrap_or(false),
            })
            .collect()
    }

    #[test]
    fn init_layouts_match_the_committed_fixture() {
        // The on-chain flag specs must equal what the fixture/IDL define.
        assert_eq!(
            fixture_layout("initUserMetadata"),
            KAMINO_INIT_USER_METADATA_LAYOUT.to_vec()
        );
        assert_eq!(
            fixture_layout("initObligation"),
            KAMINO_INIT_OBLIGATION_LAYOUT.to_vec()
        );
    }

    #[test]
    fn correct_layout_matches() {
        assert!(account_layout_matches(
            &KAMINO_INIT_USER_METADATA_LAYOUT,
            &KAMINO_INIT_USER_METADATA_LAYOUT,
        ));
    }

    #[test]
    fn missing_account_layout_fails_loudly() {
        // A short slice (missing the trailing systemProgram) must not match.
        assert!(!account_layout_matches(
            &KAMINO_INIT_OBLIGATION_LAYOUT,
            &KAMINO_INIT_OBLIGATION_LAYOUT[..8],
        ));
    }

    #[test]
    fn wrong_signer_flag_layout_fails_loudly() {
        let mut bad = KAMINO_INIT_USER_METADATA_LAYOUT;
        bad[0].is_signer = false; // owner must remain a signer
        assert!(!account_layout_matches(&KAMINO_INIT_USER_METADATA_LAYOUT, &bad));
    }

    #[test]
    fn state_pda_derivation_is_deterministic() {
        let id = [7u8; 32];
        assert_eq!(adapter_state_pda(&id, &crate::ID), adapter_state_pda(&id, &crate::ID));
    }
}
