#![allow(deprecated, unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("CjGjc5uAnEuXBfRc9NKiNTxcvjxZA9snZ3V1MqKvJpoY");

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
        state.bump = ctx.bumps.state;
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

        initialize_position_if_needed(
            &mut ctx.accounts.position,
            ctx.accounts.user.key(),
            adapter_id,
            ctx.bumps.position,
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
    #[account(
        init,
        payer = payer,
        space = AdapterState::SPACE,
        seeds = [ADAPTER_SEED, adapter_id.as_ref()],
        bump
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
    #[account(mut, seeds = [ADAPTER_SEED, adapter_id.as_ref()], bump = state.bump)]
    pub state: Account<'info, AdapterState>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct AdapterRoute<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [ADAPTER_SEED, adapter_id.as_ref()], bump = state.bump)]
    pub state: Account<'info, AdapterState>,
    #[account(
        init_if_needed,
        payer = user,
        space = Position::SPACE,
        seeds = [POSITION_SEED, adapter_id.as_ref(), user.key().as_ref()],
        bump
    )]
    pub position: Account<'info, Position>,
    pub system_program: Program<'info, System>,
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
}
