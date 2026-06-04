#![allow(deprecated, unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

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

    /// Kamino USDC real-CPI deposit mutation.
    ///
    /// Refresh instructions remain top-level sibling klend instructions built by
    /// the client/dispatcher. This entrypoint only performs the PDA-signed
    /// mutation path: user USDC -> adapter vault, then klend deposit from that
    /// vault into the state-PDA-owned obligation.
    pub fn kamino_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
        adapter_id: [u8; 32],
        amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        require!(amount > 0, AdapterError::InvalidAmount);
        guard_kamino_cpi_route(&ctx, adapter_id)?;
        validate_kamino_deposit_accounts(&ctx)?;

        let shares_out = quote_deposit_shares(&ctx.accounts.state, amount)?;
        require!(shares_out >= min_shares_out, AdapterError::SlippageExceeded);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_underlying.to_account_info(),
                    to: ctx.accounts.adapter_underlying.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        let bump = ctx.accounts.state.bump;
        let signer_seeds: &[&[u8]] = &[
            ADAPTER_SEED,
            adapter_id.as_ref(),
            core::slice::from_ref(&bump),
        ];
        let signer = &[signer_seeds];
        let mut deposit_data =
            anchor_sighash("deposit_reserve_liquidity_and_obligation_collateral").to_vec();
        deposit_data.extend_from_slice(&amount.to_le_bytes());
        invoke_signed(
            &Instruction {
                program_id: KAMINO_PROGRAM_ID,
                accounts: remaining_accounts_to_metas(
                    ctx.remaining_accounts,
                    &KAMINO_DEPOSIT_LAYOUT,
                ),
                data: deposit_data,
            },
            ctx.remaining_accounts,
            signer,
        )?;

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
        ctx.accounts.state.last_update_slot = Clock::get()?.slot;

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

    /// Kamino USDC real-CPI full withdraw mutation.
    ///
    /// This path validates the canonical klend withdraw account slice, redeems
    /// the entire state-PDA-owned obligation with Kamino's `u64::MAX` convention,
    /// then transfers the redeemed USDC from the adapter vault to the user. It
    /// intentionally rejects partial withdraws until the adapter decodes refreshed
    /// reserve/obligation state and can convert USDC value to cToken collateral
    /// amounts safely.
    pub fn kamino_withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
        adapter_id: [u8; 32],
        shares: u64,
        min_assets_out: u64,
    ) -> Result<()> {
        require!(shares > 0, AdapterError::InvalidAmount);
        guard_kamino_cpi_route(&ctx, adapter_id)?;
        validate_kamino_withdraw_accounts(&ctx)?;
        require_keys_eq!(
            ctx.accounts.position.owner,
            ctx.accounts.user.key(),
            AdapterError::Unauthorized
        );
        require!(
            ctx.accounts.position.adapter_id == adapter_id,
            AdapterError::AdapterMismatch
        );
        require!(
            ctx.accounts.position.shares >= shares,
            AdapterError::InsufficientShares
        );

        let quoted_assets_out = quote_withdraw_assets(&ctx.accounts.state, shares)?;
        require!(
            quoted_assets_out >= min_assets_out,
            AdapterError::SlippageExceeded
        );
        let collateral_amount = kamino_full_withdraw_collateral_amount(
            ctx.accounts.position.shares,
            ctx.accounts.state.total_shares,
            shares,
        )?;
        let vault_before = ctx.accounts.adapter_underlying.amount;

        let bump = ctx.accounts.state.bump;
        let signer_seeds: &[&[u8]] = &[
            ADAPTER_SEED,
            adapter_id.as_ref(),
            core::slice::from_ref(&bump),
        ];
        let signer = &[signer_seeds];
        let mut withdraw_data =
            anchor_sighash("withdraw_obligation_collateral_and_redeem_reserve_collateral").to_vec();
        withdraw_data.extend_from_slice(&collateral_amount.to_le_bytes());
        invoke_signed(
            &Instruction {
                program_id: KAMINO_PROGRAM_ID,
                accounts: remaining_accounts_to_metas(
                    ctx.remaining_accounts,
                    &KAMINO_WITHDRAW_LAYOUT,
                ),
                data: withdraw_data,
            },
            ctx.remaining_accounts,
            signer,
        )?;

        ctx.accounts.adapter_underlying.reload()?;
        let redeemed = ctx
            .accounts
            .adapter_underlying
            .amount
            .checked_sub(vault_before)
            .ok_or(AdapterError::MathOverflow)?;
        require!(redeemed > 0, AdapterError::SlippageExceeded);
        require!(redeemed >= min_assets_out, AdapterError::SlippageExceeded);

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.adapter_underlying.to_account_info(),
                    to: ctx.accounts.user_underlying.to_account_info(),
                    authority: ctx.accounts.state.to_account_info(),
                },
                signer,
            ),
            redeemed,
        )?;

        ctx.accounts.position.shares = 0;
        ctx.accounts.state.total_shares = 0;
        ctx.accounts.state.total_assets = 0;
        ctx.accounts.state.last_update_slot = Clock::get()?.slot;

        update_position_value(&ctx.accounts.state, &mut ctx.accounts.position)?;

        emit!(AdapterWithdraw {
            adapter_id,
            user: ctx.accounts.user.key(),
            shares,
            assets_out: redeemed,
            position_value: ctx.accounts.position.last_value_assets,
        });

        Ok(())
    }

    /// First-deposit init path for the Kamino USDC adapter.
    ///
    /// Performs the two PDA-signed klend setup CPIs — `initUserMetadata` then
    /// `initObligation` — owned by the adapter `state` PDA (the pooled obligation
    /// owner). This is split out from deposit/withdraw per docs/kamino-cpi-design.md;
    /// it moves no funds and implements no deposit/withdraw/value CPI. Signer seeds
    /// are the approved `[b"adapter", adapter_id, &[state.bump]]`. Not idempotent:
    /// klend rejects re-init, so callers run this only on first use.
    pub fn kamino_init(ctx: Context<KaminoInit>, adapter_id: [u8; 32]) -> Result<()> {
        let state = &ctx.accounts.state;
        require!(state.adapter_id == adapter_id, AdapterError::AdapterMismatch);
        require!(!state.paused, AdapterError::Paused);
        require!(
            state.protocol == ProtocolKind::KaminoUsdc as u8,
            AdapterError::InvalidProtocol
        );

        let bump = state.bump;
        let state_key = state.key();
        let user_key = ctx.accounts.user.key();
        let signer_seeds: &[&[u8]] =
            &[ADAPTER_SEED, adapter_id.as_ref(), core::slice::from_ref(&bump)];
        let signer = &[signer_seeds];
        let klend = ctx.accounts.klend_program.key();

        // 1) initUserMetadata(userLookupTable = Pubkey::default()).
        let mut um_data = anchor_sighash("init_user_metadata").to_vec();
        um_data.extend_from_slice(Pubkey::default().as_ref());
        let um_metas = vec![
            AccountMeta::new_readonly(state_key, true), // owner (state PDA)
            AccountMeta::new(user_key, true),           // feePayer
            AccountMeta::new(ctx.accounts.user_metadata.key(), false),
            AccountMeta::new_readonly(ctx.accounts.referrer_user_metadata.key(), false),
            AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
            AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
        ];
        require!(
            account_layout_matches(&KAMINO_INIT_USER_METADATA_LAYOUT, &metas_to_specs(&um_metas)),
            AdapterError::MissingCpiAccounts
        );
        invoke_signed(
            &Instruction { program_id: klend, accounts: um_metas, data: um_data },
            &[
                ctx.accounts.state.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.user_metadata.to_account_info(),
                ctx.accounts.referrer_user_metadata.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.klend_program.to_account_info(),
            ],
            signer,
        )?;

        // 2) initObligation(InitObligationArgs { tag: 0, id: 0 })  (Vanilla).
        let mut ob_data = anchor_sighash("init_obligation").to_vec();
        ob_data.push(0u8); // tag
        ob_data.push(0u8); // id
        let ob_metas = vec![
            AccountMeta::new_readonly(state_key, true), // obligationOwner (state PDA)
            AccountMeta::new(user_key, true),           // feePayer
            AccountMeta::new(ctx.accounts.obligation.key(), false),
            AccountMeta::new_readonly(ctx.accounts.lending_market.key(), false),
            AccountMeta::new_readonly(ctx.accounts.seed1_account.key(), false),
            AccountMeta::new_readonly(ctx.accounts.seed2_account.key(), false),
            AccountMeta::new_readonly(ctx.accounts.user_metadata.key(), false),
            AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
            AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
        ];
        require!(
            account_layout_matches(&KAMINO_INIT_OBLIGATION_LAYOUT, &metas_to_specs(&ob_metas)),
            AdapterError::MissingCpiAccounts
        );
        invoke_signed(
            &Instruction { program_id: klend, accounts: ob_metas, data: ob_data },
            &[
                ctx.accounts.state.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.obligation.to_account_info(),
                ctx.accounts.lending_market.to_account_info(),
                ctx.accounts.seed1_account.to_account_info(),
                ctx.accounts.seed2_account.to_account_info(),
                ctx.accounts.user_metadata.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.klend_program.to_account_info(),
            ],
            signer,
        )?;

        emit!(KaminoInitialized {
            adapter_id,
            obligation_owner: state_key,
            obligation: ctx.accounts.obligation.key(),
            user_metadata: ctx.accounts.user_metadata.key(),
        });
        Ok(())
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

fn guard_kamino_cpi_route<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
    adapter_id: [u8; 32],
) -> Result<()> {
    guard_cpi_route(ctx, adapter_id)?;
    require!(
        adapter_id == KAMINO_USDC_ADAPTER_ID,
        AdapterError::AdapterMismatch
    );
    require!(
        ctx.accounts.state.protocol == ProtocolKind::KaminoUsdc as u8,
        AdapterError::InvalidProtocol
    );
    require_keys_eq!(
        ctx.accounts.state.protocol_market,
        KAMINO_MAIN_MARKET,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        ctx.accounts.state.underlying_mint,
        USDC_MINT,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        ctx.accounts.user_underlying.mint,
        ctx.accounts.state.underlying_mint,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        ctx.accounts.adapter_underlying.mint,
        ctx.accounts.state.underlying_mint,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        ctx.accounts.adapter_underlying.owner,
        ctx.accounts.state.key(),
        AdapterError::Unauthorized
    );
    require_keys_eq!(
        ctx.accounts.adapter_underlying.key(),
        adapter_underlying_vault_pda(
            &ctx.accounts.state.key(),
            &ctx.accounts.token_program.key(),
            &ctx.accounts.state.underlying_mint
        ),
        AdapterError::AdapterMismatch
    );
    Ok(())
}

fn validate_kamino_deposit_accounts<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
) -> Result<()> {
    require!(
        account_info_layout_matches(
            &KAMINO_DEPOSIT_LAYOUT,
            &remaining_accounts_to_specs(ctx.remaining_accounts)
        ),
        AdapterError::MissingCpiAccounts
    );

    let accounts = ctx.remaining_accounts;
    require_keys_eq!(
        accounts[0].key(),
        ctx.accounts.state.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[2].key(),
        ctx.accounts.state.protocol_market,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[1].key(),
        KAMINO_USDC_OBLIGATION,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[3].key(),
        KAMINO_MAIN_MARKET_AUTHORITY,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[4].key(),
        KAMINO_USDC_RESERVE,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[5].key(),
        ctx.accounts.state.underlying_mint,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(accounts[5].key(), USDC_MINT, AdapterError::AdapterMismatch);
    require_keys_eq!(
        accounts[6].key(),
        KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[7].key(),
        KAMINO_USDC_RESERVE_COLLATERAL_MINT,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[8].key(),
        KAMINO_USDC_RESERVE_DESTINATION_COLLATERAL,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[9].key(),
        ctx.accounts.adapter_underlying.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[10].key(),
        KAMINO_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(accounts[10].executable, AdapterError::MissingCpiAccounts);
    require_keys_eq!(
        accounts[11].key(),
        ctx.accounts.token_program.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[12].key(),
        ctx.accounts.token_program.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[13].key(),
        anchor_lang::solana_program::sysvar::instructions::ID,
        AdapterError::AdapterMismatch
    );
    Ok(())
}

fn validate_kamino_withdraw_accounts<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
) -> Result<()> {
    require!(
        account_info_layout_matches(
            &KAMINO_WITHDRAW_LAYOUT,
            &remaining_accounts_to_specs(ctx.remaining_accounts)
        ),
        AdapterError::MissingCpiAccounts
    );

    let accounts = ctx.remaining_accounts;
    require_keys_eq!(
        accounts[0].key(),
        ctx.accounts.state.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[1].key(),
        KAMINO_USDC_OBLIGATION,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[2].key(),
        ctx.accounts.state.protocol_market,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[3].key(),
        KAMINO_MAIN_MARKET_AUTHORITY,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[4].key(),
        KAMINO_USDC_RESERVE,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[5].key(),
        ctx.accounts.state.underlying_mint,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(accounts[5].key(), USDC_MINT, AdapterError::AdapterMismatch);
    require_keys_eq!(
        accounts[6].key(),
        KAMINO_USDC_RESERVE_DESTINATION_COLLATERAL,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[7].key(),
        KAMINO_USDC_RESERVE_COLLATERAL_MINT,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[8].key(),
        KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[9].key(),
        ctx.accounts.adapter_underlying.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[10].key(),
        KAMINO_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(accounts[10].executable, AdapterError::MissingCpiAccounts);
    require_keys_eq!(
        accounts[11].key(),
        ctx.accounts.token_program.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[12].key(),
        ctx.accounts.token_program.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[13].key(),
        anchor_lang::solana_program::sysvar::instructions::ID,
        AdapterError::AdapterMismatch
    );
    Ok(())
}

pub fn kamino_full_withdraw_collateral_amount(
    position_shares: u64,
    total_shares: u64,
    shares: u64,
) -> Result<u64> {
    require!(shares > 0, AdapterError::InvalidAmount);
    require!(
        shares == position_shares && shares == total_shares,
        AdapterError::KaminoPartialWithdrawUnsupported
    );
    Ok(u64::MAX)
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

/// klend `depositReserveLiquidityAndObligationCollateral` account layout (14 accounts).
pub const KAMINO_DEPOSIT_LAYOUT: [AccountLayoutSpec; 14] = [
    spec(true, true),   // owner
    spec(false, true),  // obligation
    spec(false, false), // lendingMarket
    spec(false, false), // lendingMarketAuthority
    spec(false, true),  // reserve
    spec(false, true),  // reserveLiquidityMint
    spec(false, true),  // reserveLiquiditySupply
    spec(false, true),  // reserveCollateralMint
    spec(false, true),  // reserveDestinationDepositCollateral
    spec(false, true),  // userSourceLiquidity
    spec(false, false), // placeholderUserDestinationCollateral
    spec(false, false), // collateralTokenProgram
    spec(false, false), // liquidityTokenProgram
    spec(false, false), // instructionSysvarAccount
];

/// klend `withdrawObligationCollateralAndRedeemReserveCollateral` account layout (14 accounts).
pub const KAMINO_WITHDRAW_LAYOUT: [AccountLayoutSpec; 14] = [
    spec(true, true),   // owner
    spec(false, true),  // obligation
    spec(false, false), // lendingMarket
    spec(false, false), // lendingMarketAuthority
    spec(false, true),  // withdrawReserve
    spec(false, true),  // reserveLiquidityMint
    spec(false, true),  // reserveSourceCollateral
    spec(false, true),  // reserveCollateralMint
    spec(false, true),  // reserveLiquiditySupply
    spec(false, true),  // userDestinationLiquidity
    spec(false, false), // placeholderUserDestinationCollateral
    spec(false, false), // collateralTokenProgram
    spec(false, false), // liquidityTokenProgram
    spec(false, false), // instructionSysvarAccount
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

pub fn adapter_underlying_vault_pda(
    state: &Pubkey,
    token_program: &Pubkey,
    underlying_mint: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[state.as_ref(), token_program.as_ref(), underlying_mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

/// klend (Kamino lending) mainnet program id. The Kamino CPI target.
pub const KAMINO_PROGRAM_ID: Pubkey =
    anchor_lang::solana_program::pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    anchor_lang::solana_program::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const KAMINO_USDC_ADAPTER_ID: [u8; 32] = [
    0xc9, 0xe8, 0x01, 0xe5, 0xbc, 0x5b, 0x06, 0xd3, 0xda, 0x15, 0x65, 0xb3, 0x42, 0x99, 0xaa, 0x70,
    0xf7, 0xa4, 0x76, 0xb8, 0x45, 0x66, 0x65, 0xa0, 0x2e, 0x62, 0xbc, 0xae, 0xc5, 0x98, 0xc7, 0xa4,
];
pub const KAMINO_USDC_OBLIGATION: Pubkey =
    anchor_lang::solana_program::pubkey!("BMVjGznYqketbFdniGVSjmghmUduqYvsspnqXvpz9Maa");
pub const KAMINO_MAIN_MARKET: Pubkey =
    anchor_lang::solana_program::pubkey!("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF");
pub const KAMINO_MAIN_MARKET_AUTHORITY: Pubkey =
    anchor_lang::solana_program::pubkey!("9DrvZvyWh1HuAoZxvYWMvkf2XCzryCpGgHqrMjyDWpmo");
pub const KAMINO_USDC_RESERVE: Pubkey =
    anchor_lang::solana_program::pubkey!("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59");
pub const USDC_MINT: Pubkey =
    anchor_lang::solana_program::pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY: Pubkey =
    anchor_lang::solana_program::pubkey!("Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6");
pub const KAMINO_USDC_RESERVE_COLLATERAL_MINT: Pubkey =
    anchor_lang::solana_program::pubkey!("B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D");
pub const KAMINO_USDC_RESERVE_DESTINATION_COLLATERAL: Pubkey =
    anchor_lang::solana_program::pubkey!("3DzjXRfxRm6iejfyyMynR4tScddaanrePJ1NJU2XnPPL");

/// Anchor instruction discriminator = sha256("global:<method>")[..8].
fn anchor_sighash(method: &str) -> [u8; 8] {
    let digest = hash(format!("global:{method}").as_bytes()).to_bytes();
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

/// Project built `AccountMeta`s onto the compact layout specs the gate checks.
fn metas_to_specs(metas: &[AccountMeta]) -> Vec<AccountLayoutSpec> {
    metas
        .iter()
        .map(|m| AccountLayoutSpec {
            is_signer: m.is_signer,
            is_writable: m.is_writable,
        })
        .collect()
}

fn remaining_accounts_to_specs(accounts: &[AccountInfo]) -> Vec<AccountLayoutSpec> {
    accounts
        .iter()
        .map(|account| AccountLayoutSpec {
            is_signer: account.is_signer,
            is_writable: account.is_writable,
        })
        .collect()
}

fn account_info_layout_matches(
    expected: &[AccountLayoutSpec],
    provided: &[AccountLayoutSpec],
) -> bool {
    expected.len() == provided.len()
        && expected.iter().zip(provided).all(|(e, g)| {
            // PDA signer bits are only applied to the inner CPI AccountMeta via
            // invoke_signed, so outer AccountInfo::is_signer may be false.
            e.is_writable == g.is_writable && (e.is_signer || !g.is_signer)
        })
}

fn remaining_accounts_to_metas(
    accounts: &[AccountInfo],
    expected: &[AccountLayoutSpec],
) -> Vec<AccountMeta> {
    accounts
        .iter()
        .zip(expected.iter())
        .map(|(account, spec)| {
            if spec.is_writable {
                AccountMeta::new(account.key(), spec.is_signer)
            } else {
                AccountMeta::new_readonly(account.key(), spec.is_signer)
            }
        })
        .collect()
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

/// Accounts for the first-deposit Kamino init path. The klend-side accounts are
/// `UncheckedAccount`s validated by klend itself during the CPI; the adapter only
/// pins the klend program id and the state-PDA seeds.
#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct KaminoInit<'info> {
    /// Fee payer + transaction signer.
    #[account(mut)]
    pub user: Signer<'info>,
    /// Adapter state PDA = Kamino obligation/userMetadata owner; CPI signer.
    #[cfg_attr(
        not(feature = "idl-build"),
        account(seeds = [ADAPTER_SEED, adapter_id.as_ref()], bump = state.bump)
    )]
    #[cfg_attr(feature = "idl-build", account())]
    pub state: Account<'info, AdapterState>,
    /// CHECK: klend program, pinned by address.
    #[account(address = KAMINO_PROGRAM_ID)]
    pub klend_program: UncheckedAccount<'info>,
    /// CHECK: klend user-metadata PDA (created by initUserMetadata).
    #[account(mut)]
    pub user_metadata: UncheckedAccount<'info>,
    /// CHECK: klend obligation PDA (created by initObligation).
    #[account(mut)]
    pub obligation: UncheckedAccount<'info>,
    /// CHECK: klend lending market.
    pub lending_market: UncheckedAccount<'info>,
    /// CHECK: obligation seed1 account (Vanilla obligation).
    pub seed1_account: UncheckedAccount<'info>,
    /// CHECK: obligation seed2 account (Vanilla obligation).
    pub seed2_account: UncheckedAccount<'info>,
    /// CHECK: referrer user-metadata; optional-none placeholder = klend program id.
    pub referrer_user_metadata: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
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
pub struct KaminoInitialized {
    pub adapter_id: [u8; 32],
    pub obligation_owner: Pubkey,
    pub obligation: Pubkey,
    pub user_metadata: Pubkey,
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
    #[msg("Kamino withdraw CPI supports only full-position/full-pool redemption until reserve exchange-rate decoding is implemented")]
    KaminoPartialWithdrawUnsupported,
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

    fn fixture_pubkeys(section: &str) -> Vec<String> {
        let f: serde_json::Value =
            serde_json::from_str(FIXTURE_JSON).expect("valid Kamino CPI account-plan fixture");
        f.pointer(&format!("/accountPlan/plans/{section}/accounts"))
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("fixture missing plan section {section}"))
            .iter()
            .map(|a| a["pubkey"].as_str().expect("account pubkey").to_string())
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
        assert_eq!(
            fixture_layout("depositReserveLiquidityAndObligationCollateral"),
            KAMINO_DEPOSIT_LAYOUT.to_vec()
        );
        assert_eq!(
            fixture_layout("withdrawObligationCollateralAndRedeemReserveCollateral"),
            KAMINO_WITHDRAW_LAYOUT.to_vec()
        );
    }

    #[test]
    fn kamino_usdc_deposit_constants_match_the_committed_fixture() {
        let keys = fixture_pubkeys("depositReserveLiquidityAndObligationCollateral");
        assert_eq!(keys[1], KAMINO_USDC_OBLIGATION.to_string());
        assert_eq!(keys[2], KAMINO_MAIN_MARKET.to_string());
        assert_eq!(keys[3], KAMINO_MAIN_MARKET_AUTHORITY.to_string());
        assert_eq!(keys[4], KAMINO_USDC_RESERVE.to_string());
        assert_eq!(keys[5], USDC_MINT.to_string());
        assert_eq!(keys[6], KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY.to_string());
        assert_eq!(keys[7], KAMINO_USDC_RESERVE_COLLATERAL_MINT.to_string());
        assert_eq!(keys[8], KAMINO_USDC_RESERVE_DESTINATION_COLLATERAL.to_string());
        let (state, _) = adapter_state_pda(&KAMINO_USDC_ADAPTER_ID, &crate::ID);
        assert_eq!(
            keys[9],
            adapter_underlying_vault_pda(&state, &anchor_spl::token::ID, &USDC_MINT).to_string()
        );
        assert_eq!(keys[10], KAMINO_PROGRAM_ID.to_string());
        assert_eq!(
            keys[13],
            anchor_lang::solana_program::sysvar::instructions::ID.to_string()
        );
    }

    #[test]
    fn kamino_usdc_withdraw_constants_match_the_committed_fixture() {
        let keys = fixture_pubkeys("withdrawObligationCollateralAndRedeemReserveCollateral");
        assert_eq!(keys[1], KAMINO_USDC_OBLIGATION.to_string());
        assert_eq!(keys[2], KAMINO_MAIN_MARKET.to_string());
        assert_eq!(keys[3], KAMINO_MAIN_MARKET_AUTHORITY.to_string());
        assert_eq!(keys[4], KAMINO_USDC_RESERVE.to_string());
        assert_eq!(keys[5], USDC_MINT.to_string());
        assert_eq!(
            keys[6],
            KAMINO_USDC_RESERVE_DESTINATION_COLLATERAL.to_string()
        );
        assert_eq!(keys[7], KAMINO_USDC_RESERVE_COLLATERAL_MINT.to_string());
        assert_eq!(keys[8], KAMINO_USDC_RESERVE_LIQUIDITY_SUPPLY.to_string());
        let (state, _) = adapter_state_pda(&KAMINO_USDC_ADAPTER_ID, &crate::ID);
        assert_eq!(
            keys[9],
            adapter_underlying_vault_pda(&state, &anchor_spl::token::ID, &USDC_MINT).to_string()
        );
        assert_eq!(keys[10], KAMINO_PROGRAM_ID.to_string());
        assert_eq!(
            keys[13],
            anchor_lang::solana_program::sysvar::instructions::ID.to_string()
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
    fn remaining_account_info_layout_allows_inner_pda_signer() {
        let mut outer_infos = KAMINO_DEPOSIT_LAYOUT;
        outer_infos[0].is_signer = false; // state PDA signs only inside invoke_signed
        assert!(account_info_layout_matches(
            &KAMINO_DEPOSIT_LAYOUT,
            &outer_infos
        ));

        outer_infos[1].is_writable = false;
        assert!(!account_info_layout_matches(
            &KAMINO_DEPOSIT_LAYOUT,
            &outer_infos
        ));
    }

    #[test]
    fn full_kamino_withdraw_uses_max_collateral_amount() {
        assert_eq!(
            kamino_full_withdraw_collateral_amount(1_000, 1_000, 1_000).unwrap(),
            u64::MAX
        );
    }

    #[test]
    fn partial_kamino_withdraw_fails_loudly() {
        assert!(kamino_full_withdraw_collateral_amount(1_000, 2_000, 1_000).is_err());
        assert!(kamino_full_withdraw_collateral_amount(1_000, 1_000, 500).is_err());
    }

    #[test]
    fn state_pda_derivation_is_deterministic() {
        let id = [7u8; 32];
        assert_eq!(adapter_state_pda(&id, &crate::ID), adapter_state_pda(&id, &crate::ID));
    }

    #[test]
    fn init_sighashes_are_sized_and_distinct() {
        let um = anchor_sighash("init_user_metadata");
        let ob = anchor_sighash("init_obligation");
        let deposit = anchor_sighash("deposit_reserve_liquidity_and_obligation_collateral");
        let withdraw =
            anchor_sighash("withdraw_obligation_collateral_and_redeem_reserve_collateral");
        assert_eq!(um.len(), 8);
        assert_eq!(ob.len(), 8);
        assert_eq!(deposit.len(), 8);
        assert_eq!(withdraw.len(), 8);
        assert_eq!(deposit, [129, 199, 4, 2, 222, 39, 26, 46]);
        assert_eq!(withdraw, [75, 93, 93, 220, 34, 150, 218, 196]);
        assert_ne!(um, ob);
        assert_ne!(um, deposit);
        assert_ne!(ob, deposit);
        assert_ne!(deposit, withdraw);
    }
}
