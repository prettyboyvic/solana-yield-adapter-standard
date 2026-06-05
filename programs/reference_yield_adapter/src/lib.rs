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

    /// Real-CPI value refresh route. Supported adapters decode read-only protocol
    /// account state; unsupported adapters fail loudly with no simulation.
    pub fn current_value_cpi<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
        adapter_id: [u8; 32],
    ) -> Result<()> {
        guard_cpi_route(&ctx, adapter_id)?;
        if adapter_id == KAMINO_USDC_ADAPTER_ID {
            return kamino_current_value(ctx, adapter_id);
        }
        if adapter_id == MARGINFI_USDC_ADAPTER_ID {
            return marginfi_current_value(ctx, adapter_id);
        }
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
            anchor_sighash("deposit_reserve_liquidity_and_obligation_collateral_v2").to_vec();
        deposit_data.extend_from_slice(&amount.to_le_bytes());
        invoke_signed(
            &Instruction {
                program_id: KAMINO_PROGRAM_ID,
                accounts: remaining_accounts_to_metas(
                    ctx.remaining_accounts,
                    &KAMINO_DEPOSIT_V2_LAYOUT,
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
            anchor_sighash("withdraw_obligation_collateral_and_redeem_reserve_collateral_v2")
                .to_vec();
        withdraw_data.extend_from_slice(&collateral_amount.to_le_bytes());
        invoke_signed(
            &Instruction {
                program_id: KAMINO_PROGRAM_ID,
                accounts: remaining_accounts_to_metas(
                    ctx.remaining_accounts,
                    &KAMINO_WITHDRAW_V2_LAYOUT,
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

    /// Initialize a fresh runtime-keypair MarginFi account owned by the adapter
    /// state PDA. This moves no funds and does not wire deposit/withdraw CPI.
    pub fn marginfi_init(ctx: Context<MarginfiInit>, adapter_id: [u8; 32]) -> Result<()> {
        guard_marginfi_init_state(&ctx.accounts.state, adapter_id)?;

        let bump = ctx.accounts.state.bump;
        let signer_seeds: &[&[u8]] =
            &[ADAPTER_SEED, adapter_id.as_ref(), core::slice::from_ref(&bump)];
        let signer = &[signer_seeds];
        let metas = marginfi_init_account_metas(
            ctx.accounts.marginfi_group.key(),
            ctx.accounts.marginfi_account.key(),
            ctx.accounts.state.key(),
            ctx.accounts.user.key(),
            ctx.accounts.system_program.key(),
        );
        require!(
            account_layout_matches(&MARGINFI_ACCOUNT_INITIALIZE_LAYOUT, &metas_to_specs(&metas)),
            AdapterError::MissingCpiAccounts
        );

        invoke_signed(
            &Instruction {
                program_id: MARGINFI_PROGRAM_ID,
                accounts: metas,
                data: anchor_sighash("marginfi_account_initialize").to_vec(),
            },
            &[
                ctx.accounts.marginfi_group.to_account_info(),
                ctx.accounts.marginfi_account.to_account_info(),
                ctx.accounts.state.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.marginfi_program.to_account_info(),
            ],
            signer,
        )?;

        emit!(MarginfiInitialized {
            adapter_id,
            marginfi_account: ctx.accounts.marginfi_account.key(),
            authority: ctx.accounts.state.key(),
        });
        Ok(())
    }

    /// MarginFi USDC real-CPI deposit mutation.
    ///
    /// The user funds the adapter's state-PDA-owned USDC vault first, then the
    /// state PDA signs MarginFi's `lending_account_deposit` CPI from that vault.
    /// `remaining_accounts` contains the seven committed IDL metas followed by
    /// the executable MarginFi program account required by `invoke_signed`.
    pub fn marginfi_deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
        adapter_id: [u8; 32],
        amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        require!(amount > 0, AdapterError::InvalidAmount);
        guard_marginfi_deposit_route(&ctx, adapter_id)?;
        validate_marginfi_deposit_accounts(&ctx)?;

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
        let signer_seeds: &[&[u8]] =
            &[ADAPTER_SEED, adapter_id.as_ref(), core::slice::from_ref(&bump)];
        let signer = &[signer_seeds];
        let accounts = &ctx.remaining_accounts[..MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT.len()];
        invoke_signed(
            &Instruction {
                program_id: MARGINFI_PROGRAM_ID,
                accounts: marginfi_deposit_account_metas(
                    accounts[0].key(),
                    accounts[1].key(),
                    accounts[2].key(),
                    accounts[3].key(),
                    accounts[4].key(),
                    accounts[5].key(),
                    accounts[6].key(),
                ),
                data: marginfi_deposit_instruction_data(amount),
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

fn guard_marginfi_init_state(state: &AdapterState, adapter_id: [u8; 32]) -> Result<()> {
    require!(state.adapter_id == adapter_id, AdapterError::AdapterMismatch);
    require!(!state.paused, AdapterError::Paused);
    require!(
        adapter_id == MARGINFI_USDC_ADAPTER_ID,
        AdapterError::AdapterMismatch
    );
    require!(
        state.protocol == ProtocolKind::MarginfiUsdc as u8,
        AdapterError::InvalidProtocol
    );
    require_keys_eq!(
        state.protocol_market,
        MARGINFI_PRODUCTION_GROUP,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        state.underlying_mint,
        USDC_MINT,
        AdapterError::AdapterMismatch
    );
    Ok(())
}

fn guard_marginfi_deposit_state(state: &AdapterState, adapter_id: [u8; 32]) -> Result<()> {
    guard_marginfi_init_state(state, adapter_id)
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

fn guard_marginfi_deposit_route<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
    adapter_id: [u8; 32],
) -> Result<()> {
    guard_cpi_route(ctx, adapter_id)?;
    guard_marginfi_deposit_state(&ctx.accounts.state, adapter_id)?;
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

fn guard_marginfi_current_value_route<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
    adapter_id: [u8; 32],
) -> Result<()> {
    guard_cpi_route(ctx, adapter_id)?;
    require!(
        adapter_id == MARGINFI_USDC_ADAPTER_ID,
        AdapterError::AdapterMismatch
    );
    require!(
        ctx.accounts.state.protocol == ProtocolKind::MarginfiUsdc as u8,
        AdapterError::InvalidProtocol
    );
    require_keys_eq!(
        ctx.accounts.state.protocol_market,
        MARGINFI_PRODUCTION_GROUP,
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

fn validate_marginfi_deposit_accounts<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
) -> Result<()> {
    let accounts = ctx.remaining_accounts;
    require!(
        accounts.len() == MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT.len() + 1,
        AdapterError::MissingCpiAccounts
    );
    let instruction_accounts = &accounts[..MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT.len()];
    require!(
        account_info_layout_matches(
            &marginfi_deposit_outer_account_layout(),
            &remaining_accounts_to_specs(instruction_accounts)
        ),
        AdapterError::MissingCpiAccounts
    );

    require_keys_eq!(
        instruction_accounts[0].key(),
        MARGINFI_PRODUCTION_GROUP,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        instruction_accounts[2].key(),
        ctx.accounts.state.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        instruction_accounts[3].key(),
        MARGINFI_USDC_BANK,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        instruction_accounts[4].key(),
        ctx.accounts.adapter_underlying.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        instruction_accounts[5].key(),
        MARGINFI_USDC_LIQUIDITY_VAULT,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        instruction_accounts[6].key(),
        ctx.accounts.token_program.key(),
        AdapterError::AdapterMismatch
    );
    require!(
        instruction_accounts[0].owner == &MARGINFI_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(
        instruction_accounts[1].owner == &MARGINFI_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(
        instruction_accounts[3].owner == &MARGINFI_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        *instruction_accounts[4].owner,
        ctx.accounts.token_program.key(),
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        *instruction_accounts[5].owner,
        ctx.accounts.token_program.key(),
        AdapterError::AdapterMismatch
    );
    require!(instruction_accounts[6].executable, AdapterError::MissingCpiAccounts);

    let marginfi_program = &accounts[MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT.len()];
    require_keys_eq!(
        marginfi_program.key(),
        MARGINFI_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(marginfi_program.executable, AdapterError::MissingCpiAccounts);
    Ok(())
}

fn validate_kamino_deposit_accounts<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
) -> Result<()> {
    require!(
        account_info_layout_matches(
            &KAMINO_DEPOSIT_V2_LAYOUT,
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
    require_keys_eq!(
        accounts[14].key(),
        KAMINO_USDC_OBLIGATION_FARM_STATE,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[15].key(),
        KAMINO_USDC_RESERVE_COLLATERAL_FARM_STATE,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[16].key(),
        KAMINO_FARMS_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(accounts[16].executable, AdapterError::MissingCpiAccounts);
    Ok(())
}

fn validate_kamino_withdraw_accounts<'info>(
    ctx: &Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
) -> Result<()> {
    require!(
        account_info_layout_matches(
            &KAMINO_WITHDRAW_V2_LAYOUT,
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
    require_keys_eq!(
        accounts[14].key(),
        KAMINO_USDC_OBLIGATION_FARM_STATE,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[15].key(),
        KAMINO_USDC_RESERVE_COLLATERAL_FARM_STATE,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        accounts[16].key(),
        KAMINO_FARMS_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(accounts[16].executable, AdapterError::MissingCpiAccounts);
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

/// Expected (is_signer, is_writable) metadata for one protocol instruction
/// account slot. The on-chain program intentionally stores only these compact
/// flag specs as the routing gate's expectation; it never embeds fixture JSON.
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

/// MarginFi `marginfi_account_initialize` account layout (5 accounts).
pub const MARGINFI_ACCOUNT_INITIALIZE_LAYOUT: [AccountLayoutSpec; 5] = [
    spec(false, false), // marginfi_group
    spec(true, true),   // marginfi_account
    spec(true, false),  // authority (state PDA)
    spec(true, true),   // fee_payer
    spec(false, false), // system_program
];

/// MarginFi `lending_account_deposit` IDL account layout (7 instruction metas).
pub const MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT: [AccountLayoutSpec; 7] = [
    spec(false, false), // group
    spec(false, true),  // marginfi_account
    spec(true, false),  // authority (state PDA)
    spec(false, true),  // bank
    spec(false, true),  // signer_token_account
    spec(false, true),  // liquidity_vault
    spec(false, false), // token_program
];

fn marginfi_deposit_outer_account_layout() -> [AccountLayoutSpec; 7] {
    let mut layout = MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT;
    // The same state PDA is a writable fixed AdapterCpiRoute account because
    // local accounting is updated after CPI, so its duplicate outer AccountInfo
    // is privilege-promoted. The generated inner MarginFi meta stays readonly.
    layout[2].is_writable = true;
    layout
}

/// klend `depositReserveLiquidityAndObligationCollateralV2` account layout (17 accounts).
pub const KAMINO_DEPOSIT_V2_LAYOUT: [AccountLayoutSpec; 17] = [
    spec(true, true),   // owner
    spec(false, true),  // obligation
    spec(false, false), // lendingMarket
    spec(false, false), // lendingMarketAuthority
    spec(false, true),  // reserve
    spec(false, false), // reserveLiquidityMint
    spec(false, true),  // reserveLiquiditySupply
    spec(false, true),  // reserveCollateralMint
    spec(false, true),  // reserveDestinationDepositCollateral
    spec(false, true),  // userSourceLiquidity
    spec(false, false), // placeholderUserDestinationCollateral
    spec(false, false), // collateralTokenProgram
    spec(false, false), // liquidityTokenProgram
    spec(false, false), // instructionSysvarAccount
    spec(false, true),  // obligationFarmUserState
    spec(false, true),  // reserveFarmState
    spec(false, false), // farmsProgram
];

/// klend `withdrawObligationCollateralAndRedeemReserveCollateralV2` account layout (17 accounts).
pub const KAMINO_WITHDRAW_V2_LAYOUT: [AccountLayoutSpec; 17] = [
    spec(true, true),   // owner
    spec(false, true),  // obligation
    spec(false, false), // lendingMarket
    spec(false, false), // lendingMarketAuthority
    spec(false, true),  // withdrawReserve
    spec(false, false), // reserveLiquidityMint
    spec(false, true),  // reserveSourceCollateral
    spec(false, true),  // reserveCollateralMint
    spec(false, true),  // reserveLiquiditySupply
    spec(false, true),  // userDestinationLiquidity
    spec(false, false), // placeholderUserDestinationCollateral
    spec(false, false), // collateralTokenProgram
    spec(false, false), // liquidityTokenProgram
    spec(false, false), // instructionSysvarAccount
    spec(false, true),  // obligationFarmUserState
    spec(false, true),  // reserveFarmState
    spec(false, false), // farmsProgram
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
pub const KAMINO_FARMS_PROGRAM_ID: Pubkey =
    anchor_lang::solana_program::pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
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
pub const KAMINO_USDC_RESERVE_COLLATERAL_FARM_STATE: Pubkey =
    anchor_lang::solana_program::pubkey!("JAvnB9AKtgPsTEoKmn24Bq64UMoYcrtWtq42HHBdsPkh");
pub const KAMINO_USDC_OBLIGATION_FARM_STATE: Pubkey =
    anchor_lang::solana_program::pubkey!("FpvYH3vrip5Zaj5C2YPF6hGC17rPC5FLNEzt1ZPBmDzy");
pub const MARGINFI_USDC_ADAPTER_ID: [u8; 32] = [
    0x25, 0x8d, 0x1c, 0x90, 0x9d, 0x86, 0x0b, 0x4a, 0x66, 0x11, 0x4a, 0x7f, 0x2c, 0x16, 0x7e, 0xc3,
    0x24, 0xce, 0xe4, 0x09, 0x05, 0xea, 0x3e, 0xad, 0xa1, 0x89, 0x02, 0xb9, 0xdd, 0x58, 0x3d, 0x58,
];
pub const MARGINFI_PROGRAM_ID: Pubkey =
    anchor_lang::solana_program::pubkey!("MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA");
pub const MARGINFI_PRODUCTION_GROUP: Pubkey =
    anchor_lang::solana_program::pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8");
pub const MARGINFI_USDC_BANK: Pubkey =
    anchor_lang::solana_program::pubkey!("2s37akK2eyBbp8DZgCm7RtsaEz8eJP3Nxd4urLHQv7yB");
pub const MARGINFI_USDC_LIQUIDITY_VAULT: Pubkey =
    anchor_lang::solana_program::pubkey!("7jaiZR5Sk8hdYN9MxTpczTcwbWpb5WEoxSANuUwveuat");

// ---------------------------------------------------------------------------
// Kamino (klend) on-chain account decoding.
//
// Byte offsets and the collateral->liquidity formula come from the official,
// version-pinned `@kamino-finance/klend-sdk@3.2.26` Borsh codegen (the same SDK
// the repo already uses for account derivation). No klend Rust crate is pulled
// in: we read only the few fixed-offset scalar fields we need. Scaled-fraction
// (`*Sf`) fields use klend's `Fraction` scale of 2^60.
//
// These helpers are pure and unit-tested against synthetic bytes. An off-chain
// oracle test (decode the same raw mainnet bytes with klend-sdk) is expected to
// agree within <= 1 lamport on the final `assets` value (option A precision:
// checked u128, reduce the scaled-fraction sum by 2^60 with a single floor).
const KAMINO_RESERVE_DISCRIMINATOR: [u8; 8] = [43, 242, 204, 202, 26, 247, 59, 127];
const KAMINO_OBLIGATION_DISCRIMINATOR: [u8; 8] = [168, 206, 141, 106, 88, 76, 172, 167];
const KAMINO_SF_SHIFT: u32 = 60;

// Reserve field offsets (raw account data, incl. the 8-byte anchor discriminator).
const RES_AVAILABLE_AMOUNT_OFF: usize = 224; // u64  -> 224..232
const RES_BORROWED_AMOUNT_SF_OFF: usize = 232; // u128 -> 232..248
const RES_ACC_PROTOCOL_FEES_SF_OFF: usize = 344; // u128 -> 344..360
const RES_ACC_REFERRER_FEES_SF_OFF: usize = 360; // u128 -> 360..376
const RES_PENDING_REFERRER_FEES_SF_OFF: usize = 376; // u128 -> 376..392
const RES_COLLATERAL_MINT_TOTAL_SUPPLY_OFF: usize = 2592; // u64 -> 2592..2600
const KAMINO_RESERVE_MIN_LEN: usize = 2600;

// Obligation field offsets.
const OBL_OWNER_OFF: usize = 64; // pubkey -> 64..96
const OBL_DEPOSITS_START: usize = 96;
const OBL_DEPOSIT_SLOT_SIZE: usize = 136;
const OBL_DEPOSIT_COUNT: usize = 8;
const OBL_DEPOSIT_RESERVE_OFF: usize = 0; // within slot: 0..32
const OBL_DEPOSIT_AMOUNT_OFF: usize = 32; // within slot: 32..40
const KAMINO_OBLIGATION_MIN_LEN: usize =
    OBL_DEPOSITS_START + OBL_DEPOSIT_COUNT * OBL_DEPOSIT_SLOT_SIZE; // 96 + 8*136 = 1184

fn read_u64_le(data: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(AdapterError::KaminoAccountDataTooShort)?;
    let bytes = data
        .get(offset..end)
        .ok_or(AdapterError::KaminoAccountDataTooShort)?;
    let array: [u8; 8] = bytes
        .try_into()
        .map_err(|_| AdapterError::KaminoAccountDataTooShort)?;
    Ok(u64::from_le_bytes(array))
}

fn read_u128_le(data: &[u8], offset: usize) -> Result<u128> {
    let end = offset
        .checked_add(16)
        .ok_or(AdapterError::KaminoAccountDataTooShort)?;
    let bytes = data
        .get(offset..end)
        .ok_or(AdapterError::KaminoAccountDataTooShort)?;
    let array: [u8; 16] = bytes
        .try_into()
        .map_err(|_| AdapterError::KaminoAccountDataTooShort)?;
    Ok(u128::from_le_bytes(array))
}

/// Decode the refreshed klend USDC `Reserve` and return
/// `(total_liquidity_supply_lamports, collateral_mint_total_supply)`.
///
/// `total_supply = available + borrowed - accProtocolFees - accReferrerFees
///   - pendingReferrerFees`, where every `*Sf` term is a 2^60-scaled fraction and
/// `available` is plain lamports. We accumulate in 2^60 scale and reduce once
/// (single floor) to stay within 1 lamport of klend-sdk's `Decimal` math while
/// using only checked `u128`.
pub fn read_kamino_reserve_total_supply_and_mint_supply(data: &[u8]) -> Result<(u128, u64)> {
    require!(
        data.len() >= KAMINO_RESERVE_MIN_LEN,
        AdapterError::KaminoAccountDataTooShort
    );
    require!(
        data[..8] == KAMINO_RESERVE_DISCRIMINATOR,
        AdapterError::KaminoBadDiscriminator
    );

    let available_sf = (read_u64_le(data, RES_AVAILABLE_AMOUNT_OFF)? as u128)
        .checked_mul(1u128 << KAMINO_SF_SHIFT)
        .ok_or(AdapterError::MathOverflow)?;
    let borrowed_sf = read_u128_le(data, RES_BORROWED_AMOUNT_SF_OFF)?;
    let acc_protocol_sf = read_u128_le(data, RES_ACC_PROTOCOL_FEES_SF_OFF)?;
    let acc_referrer_sf = read_u128_le(data, RES_ACC_REFERRER_FEES_SF_OFF)?;
    let pending_referrer_sf = read_u128_le(data, RES_PENDING_REFERRER_FEES_SF_OFF)?;
    let mint_total_supply = read_u64_le(data, RES_COLLATERAL_MINT_TOTAL_SUPPLY_OFF)?;

    let total_supply_sf = available_sf
        .checked_add(borrowed_sf)
        .ok_or(AdapterError::MathOverflow)?
        .checked_sub(acc_protocol_sf)
        .ok_or(AdapterError::KaminoTotalSupplyUnderflow)?
        .checked_sub(acc_referrer_sf)
        .ok_or(AdapterError::KaminoTotalSupplyUnderflow)?
        .checked_sub(pending_referrer_sf)
        .ok_or(AdapterError::KaminoTotalSupplyUnderflow)?;
    let total_supply = total_supply_sf >> KAMINO_SF_SHIFT;
    Ok((total_supply, mint_total_supply))
}

/// Scan the klend `Obligation` deposits for `deposit_reserve` and return the
/// deposited collateral (cToken) amount. Verifies the account discriminator and
/// that the obligation `owner` equals `expected_owner` (the adapter state PDA).
pub fn read_kamino_obligation_collateral(
    data: &[u8],
    expected_owner: Pubkey,
    deposit_reserve: Pubkey,
) -> Result<u64> {
    require!(
        data.len() >= KAMINO_OBLIGATION_MIN_LEN,
        AdapterError::KaminoAccountDataTooShort
    );
    require!(
        data[..8] == KAMINO_OBLIGATION_DISCRIMINATOR,
        AdapterError::KaminoBadDiscriminator
    );

    let owner_bytes = data
        .get(OBL_OWNER_OFF..OBL_OWNER_OFF + 32)
        .ok_or(AdapterError::KaminoAccountDataTooShort)?;
    require!(
        owner_bytes == expected_owner.as_ref(),
        AdapterError::KaminoObligationOwnerMismatch
    );

    let reserve_ref = deposit_reserve.as_ref();
    for i in 0..OBL_DEPOSIT_COUNT {
        let slot = OBL_DEPOSITS_START + i * OBL_DEPOSIT_SLOT_SIZE;
        let reserve_off = slot + OBL_DEPOSIT_RESERVE_OFF;
        let reserve_bytes = data
            .get(reserve_off..reserve_off + 32)
            .ok_or(AdapterError::KaminoAccountDataTooShort)?;
        if reserve_bytes == reserve_ref {
            return read_u64_le(data, slot + OBL_DEPOSIT_AMOUNT_OFF);
        }
    }
    err!(AdapterError::KaminoDepositReserveNotFound)
}

/// Convert deposited collateral (cTokens) to underlying assets (USDC lamports):
/// `assets = deposited * total_supply / mint_total_supply`, rounded down.
pub fn kamino_collateral_to_assets(
    deposited: u64,
    total_supply: u128,
    mint_total_supply: u64,
) -> Result<u64> {
    require!(
        mint_total_supply > 0,
        AdapterError::KaminoZeroCollateralSupply
    );
    require!(total_supply > 0, AdapterError::KaminoZeroTotalSupply);
    let assets = (deposited as u128)
        .checked_mul(total_supply)
        .ok_or(AdapterError::MathOverflow)?
        .checked_div(mint_total_supply as u128)
        .ok_or(AdapterError::MathOverflow)?;
    u64::try_from(assets).map_err(|_| AdapterError::MathOverflow.into())
}

/// Real Kamino `current_value`: decode the refreshed reserve + obligation and set
/// pooled `total_assets` to the USDC value of the obligation's collateral. This is
/// read-only against klend (no CPI/mutation of Kamino state); only the adapter's
/// own pooled accounting is updated. `remaining_accounts = [usdc_reserve, obligation]`.
fn kamino_current_value<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
    adapter_id: [u8; 32],
) -> Result<()> {
    guard_kamino_cpi_route(&ctx, adapter_id)?;

    let accounts = ctx.remaining_accounts;
    require!(accounts.len() == 2, AdapterError::MissingCpiAccounts);
    let reserve_ai = &accounts[0];
    let obligation_ai = &accounts[1];
    require_keys_eq!(
        reserve_ai.key(),
        KAMINO_USDC_RESERVE,
        AdapterError::AdapterMismatch
    );
    require_keys_eq!(
        obligation_ai.key(),
        KAMINO_USDC_OBLIGATION,
        AdapterError::AdapterMismatch
    );
    require!(
        reserve_ai.owner == &KAMINO_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(
        obligation_ai.owner == &KAMINO_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );

    let (total_supply, mint_total_supply) = {
        let data = reserve_ai.try_borrow_data()?;
        read_kamino_reserve_total_supply_and_mint_supply(&data[..])?
    };
    let deposited = {
        let data = obligation_ai.try_borrow_data()?;
        read_kamino_obligation_collateral(&data[..], ctx.accounts.state.key(), KAMINO_USDC_RESERVE)?
    };
    let total_value = kamino_collateral_to_assets(deposited, total_supply, mint_total_supply)?;

    ctx.accounts.state.total_assets = total_value;
    ctx.accounts.state.last_update_slot = Clock::get()?.slot;
    update_position_value(&ctx.accounts.state, &mut ctx.accounts.position)?;

    emit!(AdapterValue {
        adapter_id,
        user: ctx.accounts.user.key(),
        shares: ctx.accounts.position.shares,
        value_assets: ctx.accounts.position.last_value_assets,
    });
    Ok(())
}

/// Real MarginFi `current_value`: decode the USDC bank's asset share value plus
/// the state-PDA-owned MarginfiAccount balance and set pooled `total_assets`.
/// This is read-only against MarginFi; no deposit/withdraw CPI is performed.
/// `remaining_accounts = [usdc_bank, marginfi_account]`.
fn marginfi_current_value<'info>(
    ctx: Context<'_, '_, '_, 'info, AdapterCpiRoute<'info>>,
    adapter_id: [u8; 32],
) -> Result<()> {
    guard_marginfi_current_value_route(&ctx, adapter_id)?;

    let accounts = ctx.remaining_accounts;
    require!(accounts.len() == 2, AdapterError::MissingCpiAccounts);
    let bank_ai = &accounts[0];
    let marginfi_account_ai = &accounts[1];
    require_keys_eq!(
        bank_ai.key(),
        MARGINFI_USDC_BANK,
        AdapterError::AdapterMismatch
    );
    require!(
        bank_ai.owner == &MARGINFI_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );
    require!(
        marginfi_account_ai.owner == &MARGINFI_PROGRAM_ID,
        AdapterError::AdapterMismatch
    );

    let total_value = {
        let bank_data = bank_ai.try_borrow_data()?;
        let account_data = marginfi_account_ai.try_borrow_data()?;
        marginfi_current_value_from_data(&bank_data[..], &account_data[..], ctx.accounts.state.key())?
    };

    ctx.accounts.state.total_assets = total_value;
    ctx.accounts.state.last_update_slot = Clock::get()?.slot;
    update_position_value(&ctx.accounts.state, &mut ctx.accounts.position)?;

    emit!(AdapterValue {
        adapter_id,
        user: ctx.accounts.user.key(),
        shares: ctx.accounts.position.shares,
        value_assets: ctx.accounts.position.last_value_assets,
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// MarginFi (mrgn-v2) on-chain account decoding.
//
// Byte offsets are proven against `@mrgnlabs/marginfi-client-v2@6.4.2`
// (bundled IDL `marginfi 0.1.7`, program MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA).
// No marginfi Rust crate is pulled in; we read only the fixed-offset scalar
// fields needed for `current_value`. `WrappedI80F48` is a little-endian i128
// with 48 fractional bits; deposit-only accounting requires non-negative values.
//
// These helpers are pure and unit-tested against synthetic bytes (step 1: read
// path only — no marginfi entrypoints / CPI yet).
const MARGINFI_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const MARGINFI_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];

// Bank field offsets (account data, incl. the 8-byte anchor discriminator).
const BANK_MINT_OFF: usize = 8; // pubkey -> 8..40
const BANK_ASSET_SHARE_VALUE_OFF: usize = 80; // WrappedI80F48 -> 80..96 (i128 LE, 48 frac bits)
const MARGINFI_BANK_MIN_LEN: usize = 96;

// MarginfiAccount field offsets. (group is at 8..40; not read here.)
const MFA_AUTHORITY_OFF: usize = 40; // pubkey -> 40..72
const MFA_BALANCES_OFF: usize = 72;
const MFA_BALANCE_COUNT: usize = 16;
const MFA_BALANCE_SIZE: usize = 104;
const BAL_ACTIVE_OFF: usize = 0; // u8 within slot
const BAL_BANK_PK_OFF: usize = 1; // pubkey within slot -> +1..+33
const BAL_ASSET_SHARES_OFF: usize = 40; // WrappedI80F48 within slot -> +40..+56
const MARGINFI_ACCOUNT_MIN_LEN: usize =
    MFA_BALANCES_OFF + MFA_BALANCE_COUNT * MFA_BALANCE_SIZE; // 72 + 16*104 = 1736

fn read_i128_le(data: &[u8], offset: usize) -> Result<i128> {
    let end = offset
        .checked_add(16)
        .ok_or(AdapterError::MarginfiAccountDataTooShort)?;
    let bytes = data
        .get(offset..end)
        .ok_or(AdapterError::MarginfiAccountDataTooShort)?;
    let array: [u8; 16] = bytes
        .try_into()
        .map_err(|_| AdapterError::MarginfiAccountDataTooShort)?;
    Ok(i128::from_le_bytes(array))
}

/// Read a `WrappedI80F48` raw value (the i128 numerator, still scaled by 2^48)
/// and require it be non-negative — deposit-only accounting never reads a
/// negative share count or share value.
fn read_marginfi_i80f48_nonneg(data: &[u8], offset: usize) -> Result<u128> {
    let raw = read_i128_le(data, offset)?;
    require!(raw >= 0, AdapterError::MarginfiNegativeValue);
    Ok(raw as u128)
}

/// Decode a MarginFi `Bank` and return the raw `asset_share_value` I80F48
/// numerator (scaled by 2^48). Verifies the account discriminator and that the
/// bank's `mint` equals `expected_mint`.
pub fn read_marginfi_bank_asset_share_value(data: &[u8], expected_mint: Pubkey) -> Result<u128> {
    require!(
        data.len() >= MARGINFI_BANK_MIN_LEN,
        AdapterError::MarginfiAccountDataTooShort
    );
    require!(
        data[..8] == MARGINFI_BANK_DISCRIMINATOR,
        AdapterError::MarginfiBadDiscriminator
    );
    let mint = data
        .get(BANK_MINT_OFF..BANK_MINT_OFF + 32)
        .ok_or(AdapterError::MarginfiAccountDataTooShort)?;
    require!(
        mint == expected_mint.as_ref(),
        AdapterError::MarginfiBankMintMismatch
    );
    read_marginfi_i80f48_nonneg(data, BANK_ASSET_SHARE_VALUE_OFF)
}

/// Scan a MarginFi `MarginfiAccount` for the active balance backed by `usdc_bank`
/// and return its raw `asset_shares` I80F48 numerator (scaled by 2^48). Verifies
/// the account discriminator and that `authority` equals `expected_authority`
/// (the adapter state PDA). Errors if no active balance matches the bank.
pub fn read_marginfi_account_asset_shares(
    data: &[u8],
    expected_authority: Pubkey,
    usdc_bank: Pubkey,
) -> Result<u128> {
    require!(
        data.len() >= MARGINFI_ACCOUNT_MIN_LEN,
        AdapterError::MarginfiAccountDataTooShort
    );
    require!(
        data[..8] == MARGINFI_ACCOUNT_DISCRIMINATOR,
        AdapterError::MarginfiBadDiscriminator
    );
    let authority = data
        .get(MFA_AUTHORITY_OFF..MFA_AUTHORITY_OFF + 32)
        .ok_or(AdapterError::MarginfiAccountDataTooShort)?;
    require!(
        authority == expected_authority.as_ref(),
        AdapterError::MarginfiAuthorityMismatch
    );

    let bank_ref = usdc_bank.as_ref();
    for i in 0..MFA_BALANCE_COUNT {
        let slot = MFA_BALANCES_OFF + i * MFA_BALANCE_SIZE;
        let active = *data
            .get(slot + BAL_ACTIVE_OFF)
            .ok_or(AdapterError::MarginfiAccountDataTooShort)?;
        if active != 1 {
            continue;
        }
        let pk_off = slot + BAL_BANK_PK_OFF;
        let bank_pk = data
            .get(pk_off..pk_off + 32)
            .ok_or(AdapterError::MarginfiAccountDataTooShort)?;
        if bank_pk == bank_ref {
            return read_marginfi_i80f48_nonneg(data, slot + BAL_ASSET_SHARES_OFF);
        }
    }
    err!(AdapterError::MarginfiNoActiveUsdcBalance)
}

/// Full 128x128 -> 256-bit product as four little-endian u64 limbs (no deps).
fn mul_256(a: u128, b: u128) -> [u64; 4] {
    const LO64: u128 = u64::MAX as u128;
    let a0 = a & LO64;
    let a1 = a >> 64;
    let b0 = b & LO64;
    let b1 = b >> 64;
    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;

    let mut acc = p00;
    let r0 = (acc & LO64) as u64;
    acc >>= 64;
    acc += (p01 & LO64) + (p10 & LO64);
    let r1 = (acc & LO64) as u64;
    acc >>= 64;
    acc += (p01 >> 64) + (p10 >> 64) + (p11 & LO64);
    let r2 = (acc & LO64) as u64;
    acc >>= 64;
    acc += p11 >> 64;
    let r3 = (acc & LO64) as u64;
    [r0, r1, r2, r3]
}

/// Convert MarginFi shares to underlying assets (USDC lamports):
/// `assets = (asset_shares_raw * asset_share_value_raw) >> 96`, rounded down.
/// Both inputs are I80F48 numerators (each scaled by 2^48), so the product is
/// scaled by 2^96. Uses a 256-bit intermediate to avoid u128 overflow and fails
/// loudly if the result does not fit in u64.
pub fn marginfi_shares_to_assets(
    asset_shares_raw: u128,
    asset_share_value_raw: u128,
) -> Result<u64> {
    if asset_shares_raw == 0 {
        return Ok(0);
    }
    let [_r0, r1, r2, r3] = mul_256(asset_shares_raw, asset_share_value_raw);
    // result = product >> 96 must fit in u64, i.e. product < 2^160:
    require!(r3 == 0, AdapterError::MathOverflow);
    require!(r2 >> 32 == 0, AdapterError::MathOverflow);
    let assets = ((r1 as u128) >> 32) + ((r2 as u128) << 32);
    u64::try_from(assets).map_err(|_| AdapterError::MathOverflow.into())
}

pub fn marginfi_current_value_from_data(
    bank_data: &[u8],
    marginfi_account_data: &[u8],
    expected_authority: Pubkey,
) -> Result<u64> {
    let asset_share_value_raw = read_marginfi_bank_asset_share_value(bank_data, USDC_MINT)?;
    let asset_shares_raw =
        read_marginfi_account_asset_shares(marginfi_account_data, expected_authority, MARGINFI_USDC_BANK)?;
    marginfi_shares_to_assets(asset_shares_raw, asset_share_value_raw)
}

/// Anchor instruction discriminator = sha256("global:<method>")[..8].
fn anchor_sighash(method: &str) -> [u8; 8] {
    let digest = hash(format!("global:{method}").as_bytes()).to_bytes();
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

fn marginfi_init_account_metas(
    marginfi_group: Pubkey,
    marginfi_account: Pubkey,
    authority: Pubkey,
    fee_payer: Pubkey,
    system_program: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(marginfi_group, false),
        AccountMeta::new(marginfi_account, true),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(fee_payer, true),
        AccountMeta::new_readonly(system_program, false),
    ]
}

fn marginfi_deposit_account_metas(
    group: Pubkey,
    marginfi_account: Pubkey,
    authority: Pubkey,
    bank: Pubkey,
    signer_token_account: Pubkey,
    liquidity_vault: Pubkey,
    token_program: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(group, false),
        AccountMeta::new(marginfi_account, false),
        AccountMeta::new_readonly(authority, true),
        AccountMeta::new(bank, false),
        AccountMeta::new(signer_token_account, false),
        AccountMeta::new(liquidity_vault, false),
        AccountMeta::new_readonly(token_program, false),
    ]
}

fn marginfi_deposit_instruction_data(amount: u64) -> Vec<u8> {
    let mut data = anchor_sighash("lending_account_deposit").to_vec();
    data.extend_from_slice(&amount.to_le_bytes());
    data.push(0); // deposit_up_to_limit: Option<bool>::None
    data
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

/// Accounts for MarginFi `marginfi_account_initialize`. The MarginFi account is
/// a fresh runtime keypair signer; the adapter state PDA is the account authority.
#[derive(Accounts)]
#[instruction(adapter_id: [u8; 32])]
pub struct MarginfiInit<'info> {
    /// Fee payer + transaction signer.
    #[account(mut)]
    pub user: Signer<'info>,
    /// Adapter state PDA = MarginFi account authority; CPI signer.
    #[cfg_attr(
        not(feature = "idl-build"),
        account(seeds = [ADAPTER_SEED, adapter_id.as_ref()], bump = state.bump)
    )]
    #[cfg_attr(feature = "idl-build", account())]
    pub state: Account<'info, AdapterState>,
    /// CHECK: MarginFi program, pinned by address.
    #[account(address = MARGINFI_PROGRAM_ID)]
    pub marginfi_program: UncheckedAccount<'info>,
    /// CHECK: production MarginFi group, pinned by address.
    #[account(address = MARGINFI_PRODUCTION_GROUP)]
    pub marginfi_group: UncheckedAccount<'info>,
    /// Fresh runtime keypair created and owned by MarginFi during the CPI.
    #[account(mut)]
    pub marginfi_account: Signer<'info>,
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
pub struct MarginfiInitialized {
    pub adapter_id: [u8; 32],
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
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
    #[msg("Kamino account data is shorter than the expected klend layout")]
    KaminoAccountDataTooShort,
    #[msg("Kamino account discriminator does not match the expected klend account")]
    KaminoBadDiscriminator,
    #[msg("Kamino obligation owner does not match the adapter state PDA")]
    KaminoObligationOwnerMismatch,
    #[msg("Kamino obligation has no deposit for the expected reserve")]
    KaminoDepositReserveNotFound,
    #[msg("Kamino reserve collateral mint total supply is zero")]
    KaminoZeroCollateralSupply,
    #[msg("Kamino reserve total liquidity supply is zero")]
    KaminoZeroTotalSupply,
    #[msg("Kamino reserve fee reduction underflowed total liquidity supply")]
    KaminoTotalSupplyUnderflow,
    #[msg("MarginFi account data is shorter than the expected layout")]
    MarginfiAccountDataTooShort,
    #[msg("MarginFi account discriminator does not match the expected account")]
    MarginfiBadDiscriminator,
    #[msg("MarginFi I80F48 value is negative; deposit-only accounting expects non-negative")]
    MarginfiNegativeValue,
    #[msg("MarginFi bank mint does not match the expected underlying mint")]
    MarginfiBankMintMismatch,
    #[msg("MarginFi account authority does not match the adapter state PDA")]
    MarginfiAuthorityMismatch,
    #[msg("MarginFi account has no active balance for the expected USDC bank")]
    MarginfiNoActiveUsdcBalance,
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
    const MARGINFI_FIXTURE_JSON: &str =
        include_str!("../../../packages/sdk/fixtures/marginfi-cpi-account-plan.json");

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

    fn marginfi_init_fixture() -> serde_json::Value {
        let fixture: serde_json::Value = serde_json::from_str(MARGINFI_FIXTURE_JSON)
            .expect("valid MarginFi CPI account-plan fixture");
        fixture["plans"]["marginfi_account_initialize"].clone()
    }

    fn marginfi_deposit_fixture() -> serde_json::Value {
        let fixture: serde_json::Value = serde_json::from_str(MARGINFI_FIXTURE_JSON)
            .expect("valid MarginFi CPI account-plan fixture");
        fixture["plans"]["lending_account_deposit"].clone()
    }

    fn marginfi_init_state() -> AdapterState {
        AdapterState {
            adapter_id: MARGINFI_USDC_ADAPTER_ID,
            protocol: ProtocolKind::MarginfiUsdc as u8,
            authority: Pubkey::new_unique(),
            underlying_mint: USDC_MINT,
            receipt_mint: Pubkey::default(),
            protocol_market: MARGINFI_PRODUCTION_GROUP,
            value_oracle: Pubkey::default(),
            total_assets: 0,
            total_shares: 0,
            last_update_slot: 0,
            bump: adapter_state_pda(&MARGINFI_USDC_ADAPTER_ID, &crate::ID).1,
            paused: false,
            metadata_uri: String::new(),
        }
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
            fixture_layout("depositReserveLiquidityAndObligationCollateralV2"),
            KAMINO_DEPOSIT_V2_LAYOUT.to_vec()
        );
        assert_eq!(
            fixture_layout("withdrawObligationCollateralAndRedeemReserveCollateralV2"),
            KAMINO_WITHDRAW_V2_LAYOUT.to_vec()
        );
    }

    #[test]
    fn marginfi_init_layout_and_discriminator_match_committed_fixture() {
        let plan = marginfi_init_fixture();
        let accounts = plan["accounts"].as_array().expect("MarginFi init accounts");
        let layout = accounts
            .iter()
            .map(|account| AccountLayoutSpec {
                is_signer: account["isSigner"].as_bool().unwrap_or(false),
                is_writable: account["isWritable"].as_bool().unwrap_or(false),
            })
            .collect::<Vec<_>>();
        let names = accounts
            .iter()
            .map(|account| account["name"].as_str().expect("account name"))
            .collect::<Vec<_>>();
        let discriminator = plan["discriminator"]
            .as_array()
            .expect("MarginFi init discriminator")
            .iter()
            .map(|byte| byte.as_u64().expect("discriminator byte") as u8)
            .collect::<Vec<_>>();

        assert_eq!(layout, MARGINFI_ACCOUNT_INITIALIZE_LAYOUT.to_vec());
        assert_eq!(
            names,
            [
                "marginfi_group",
                "marginfi_account",
                "authority",
                "fee_payer",
                "system_program",
            ]
        );
        assert_eq!(
            discriminator,
            anchor_sighash("marginfi_account_initialize").to_vec()
        );
    }

    #[test]
    fn marginfi_init_meta_builder_matches_fixture_constants_and_flags() {
        let plan = marginfi_init_fixture();
        let runtime_account = Pubkey::new_unique();
        let fee_payer = Pubkey::new_unique();
        let (state, _) = adapter_state_pda(&MARGINFI_USDC_ADAPTER_ID, &crate::ID);
        let metas = marginfi_init_account_metas(
            MARGINFI_PRODUCTION_GROUP,
            runtime_account,
            state,
            fee_payer,
            System::id(),
        );

        assert_eq!(metas_to_specs(&metas), MARGINFI_ACCOUNT_INITIALIZE_LAYOUT);
        assert_eq!(metas[0].pubkey, MARGINFI_PRODUCTION_GROUP);
        assert_eq!(metas[1].pubkey, runtime_account);
        assert_eq!(metas[2].pubkey, state);
        assert_eq!(metas[3].pubkey, fee_payer);
        assert_eq!(metas[4].pubkey, System::id());
        assert_eq!(
            plan["accounts"][0]["pubkey"].as_str().expect("fixture group"),
            MARGINFI_PRODUCTION_GROUP.to_string()
        );
        assert_eq!(
            plan["accounts"][2]["pubkey"].as_str().expect("fixture authority"),
            state.to_string()
        );
    }

    #[test]
    fn marginfi_init_guard_accepts_only_configured_state() {
        assert!(guard_marginfi_init_state(&marginfi_init_state(), MARGINFI_USDC_ADAPTER_ID).is_ok());

        let mut paused = marginfi_init_state();
        paused.paused = true;
        assert!(guard_marginfi_init_state(&paused, MARGINFI_USDC_ADAPTER_ID).is_err());

        let mut wrong_protocol = marginfi_init_state();
        wrong_protocol.protocol = ProtocolKind::KaminoUsdc as u8;
        assert!(guard_marginfi_init_state(&wrong_protocol, MARGINFI_USDC_ADAPTER_ID).is_err());

        let mut wrong_market = marginfi_init_state();
        wrong_market.protocol_market = Pubkey::new_unique();
        assert!(guard_marginfi_init_state(&wrong_market, MARGINFI_USDC_ADAPTER_ID).is_err());

        assert!(guard_marginfi_init_state(&marginfi_init_state(), KAMINO_USDC_ADAPTER_ID).is_err());
    }

    #[test]
    fn marginfi_deposit_layout_and_discriminator_match_committed_fixture() {
        let plan = marginfi_deposit_fixture();
        let accounts = plan["accounts"].as_array().expect("MarginFi deposit accounts");
        let layout = accounts
            .iter()
            .map(|account| AccountLayoutSpec {
                is_signer: account["isSigner"].as_bool().unwrap_or(false),
                is_writable: account["isWritable"].as_bool().unwrap_or(false),
            })
            .collect::<Vec<_>>();
        let names = accounts
            .iter()
            .map(|account| account["name"].as_str().expect("account name"))
            .collect::<Vec<_>>();
        let discriminator = plan["discriminator"]
            .as_array()
            .expect("MarginFi deposit discriminator")
            .iter()
            .map(|byte| byte.as_u64().expect("discriminator byte") as u8)
            .collect::<Vec<_>>();

        assert_eq!(layout, MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT.to_vec());
        assert_eq!(
            names,
            [
                "group",
                "marginfi_account",
                "authority",
                "bank",
                "signer_token_account",
                "liquidity_vault",
                "token_program",
            ]
        );
        assert_eq!(discriminator, anchor_sighash("lending_account_deposit").to_vec());
    }

    #[test]
    fn marginfi_deposit_meta_builder_matches_fixture_constants_and_flags() {
        let plan = marginfi_deposit_fixture();
        let runtime_account = Pubkey::new_unique();
        let (state, _) = adapter_state_pda(&MARGINFI_USDC_ADAPTER_ID, &crate::ID);
        let adapter_vault =
            adapter_underlying_vault_pda(&state, &anchor_spl::token::ID, &USDC_MINT);
        let metas = marginfi_deposit_account_metas(
            MARGINFI_PRODUCTION_GROUP,
            runtime_account,
            state,
            MARGINFI_USDC_BANK,
            adapter_vault,
            MARGINFI_USDC_LIQUIDITY_VAULT,
            anchor_spl::token::ID,
        );

        assert_eq!(metas_to_specs(&metas), MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT);
        assert_eq!(
            metas.iter().map(|meta| meta.pubkey).collect::<Vec<_>>(),
            [
                MARGINFI_PRODUCTION_GROUP,
                runtime_account,
                state,
                MARGINFI_USDC_BANK,
                adapter_vault,
                MARGINFI_USDC_LIQUIDITY_VAULT,
                anchor_spl::token::ID,
            ]
        );
        assert_eq!(
            plan["accounts"][0]["pubkey"].as_str().expect("fixture group"),
            MARGINFI_PRODUCTION_GROUP.to_string()
        );
        assert_eq!(
            plan["accounts"][1]["pubkey"].as_str().expect("runtime account"),
            "GENERATED_AT_RUNTIME"
        );
        assert!(plan["accounts"][1]["generatedAtRuntime"]
            .as_bool()
            .expect("runtime marker"));
        assert_eq!(
            plan["accounts"][2]["pubkey"].as_str().expect("fixture authority"),
            state.to_string()
        );
        assert_eq!(
            plan["accounts"][3]["pubkey"].as_str().expect("fixture bank"),
            MARGINFI_USDC_BANK.to_string()
        );
        assert_eq!(
            plan["accounts"][4]["pubkey"].as_str().expect("fixture adapter vault"),
            adapter_vault.to_string()
        );
        assert_eq!(
            plan["accounts"][5]["pubkey"].as_str().expect("fixture liquidity vault"),
            MARGINFI_USDC_LIQUIDITY_VAULT.to_string()
        );
        assert_eq!(
            plan["accounts"][6]["pubkey"].as_str().expect("fixture token program"),
            anchor_spl::token::ID.to_string()
        );
    }

    #[test]
    fn marginfi_deposit_instruction_data_encodes_amount_and_none_limit() {
        let amount = 0x0807_0605_0403_0201;
        let data = marginfi_deposit_instruction_data(amount);

        assert_eq!(data.len(), 17);
        assert_eq!(&data[..8], &anchor_sighash("lending_account_deposit"));
        assert_eq!(&data[8..16], &amount.to_le_bytes());
        assert_eq!(data[16], 0);
    }

    #[test]
    fn marginfi_deposit_guard_accepts_only_configured_state() {
        assert!(
            guard_marginfi_deposit_state(&marginfi_init_state(), MARGINFI_USDC_ADAPTER_ID).is_ok()
        );

        let mut paused = marginfi_init_state();
        paused.paused = true;
        assert!(guard_marginfi_deposit_state(&paused, MARGINFI_USDC_ADAPTER_ID).is_err());

        let mut wrong_protocol = marginfi_init_state();
        wrong_protocol.protocol = ProtocolKind::KaminoUsdc as u8;
        assert!(
            guard_marginfi_deposit_state(&wrong_protocol, MARGINFI_USDC_ADAPTER_ID).is_err()
        );

        let mut wrong_market = marginfi_init_state();
        wrong_market.protocol_market = Pubkey::new_unique();
        assert!(guard_marginfi_deposit_state(&wrong_market, MARGINFI_USDC_ADAPTER_ID).is_err());

        let mut wrong_mint = marginfi_init_state();
        wrong_mint.underlying_mint = Pubkey::new_unique();
        assert!(guard_marginfi_deposit_state(&wrong_mint, MARGINFI_USDC_ADAPTER_ID).is_err());

        assert!(
            guard_marginfi_deposit_state(&marginfi_init_state(), KAMINO_USDC_ADAPTER_ID).is_err()
        );
    }

    #[test]
    fn marginfi_deposit_layout_fails_loudly_on_missing_or_wrong_accounts() {
        assert!(!account_layout_matches(
            &MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT,
            &MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT[..6],
        ));

        let mut wrong = MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT;
        wrong[3].is_writable = false;
        assert!(!account_layout_matches(
            &MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT,
            &wrong,
        ));
    }

    #[test]
    fn marginfi_deposit_outer_layout_allows_only_state_writable_promotion() {
        let outer = marginfi_deposit_outer_account_layout();
        assert!(outer[2].is_writable);
        assert!(!MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT[2].is_writable);
        assert_eq!(
            outer
                .iter()
                .enumerate()
                .filter(|(index, spec)| {
                    spec.is_writable != MARGINFI_LENDING_ACCOUNT_DEPOSIT_LAYOUT[*index].is_writable
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>(),
            [2]
        );
    }

    #[test]
    fn kamino_usdc_deposit_constants_match_the_committed_fixture() {
        let keys = fixture_pubkeys("depositReserveLiquidityAndObligationCollateralV2");
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
        assert_eq!(keys[14], KAMINO_USDC_OBLIGATION_FARM_STATE.to_string());
        assert_eq!(
            keys[15],
            KAMINO_USDC_RESERVE_COLLATERAL_FARM_STATE.to_string()
        );
        assert_eq!(keys[16], KAMINO_FARMS_PROGRAM_ID.to_string());
    }

    #[test]
    fn kamino_usdc_withdraw_constants_match_the_committed_fixture() {
        let keys = fixture_pubkeys("withdrawObligationCollateralAndRedeemReserveCollateralV2");
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
        assert_eq!(keys[14], KAMINO_USDC_OBLIGATION_FARM_STATE.to_string());
        assert_eq!(
            keys[15],
            KAMINO_USDC_RESERVE_COLLATERAL_FARM_STATE.to_string()
        );
        assert_eq!(keys[16], KAMINO_FARMS_PROGRAM_ID.to_string());
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
        let mut outer_infos = KAMINO_DEPOSIT_V2_LAYOUT;
        outer_infos[0].is_signer = false; // state PDA signs only inside invoke_signed
        assert!(account_info_layout_matches(
            &KAMINO_DEPOSIT_V2_LAYOUT,
            &outer_infos
        ));

        outer_infos[1].is_writable = false;
        assert!(!account_info_layout_matches(
            &KAMINO_DEPOSIT_V2_LAYOUT,
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

#[cfg(test)]
mod kamino_decode_tests {
    use super::*;

    fn put_u64(buf: &mut [u8], off: usize, v: u64) {
        buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }
    fn put_u128(buf: &mut [u8], off: usize, v: u128) {
        buf[off..off + 16].copy_from_slice(&v.to_le_bytes());
    }
    fn put_pubkey(buf: &mut [u8], off: usize, k: &Pubkey) {
        buf[off..off + 32].copy_from_slice(k.as_ref());
    }
    fn err_msg(e: anchor_lang::error::Error) -> String {
        match e {
            anchor_lang::error::Error::AnchorError(ae) => ae.error_msg,
            _ => String::new(),
        }
    }

    #[test]
    fn reserve_decoder_reads_exact_offsets_and_floors_once() {
        let mut data = vec![0u8; KAMINO_RESERVE_MIN_LEN];
        data[..8].copy_from_slice(&KAMINO_RESERVE_DISCRIMINATOR);
        put_u64(&mut data, RES_AVAILABLE_AMOUNT_OFF, 1_000_000);
        // 5.5 in 2^60 scale: exercises single-floor reduction of fractional bits.
        put_u128(
            &mut data,
            RES_BORROWED_AMOUNT_SF_OFF,
            (5u128 << KAMINO_SF_SHIFT) | (1u128 << (KAMINO_SF_SHIFT - 1)),
        );
        put_u128(
            &mut data,
            RES_ACC_PROTOCOL_FEES_SF_OFF,
            2u128 << KAMINO_SF_SHIFT,
        );
        put_u128(
            &mut data,
            RES_ACC_REFERRER_FEES_SF_OFF,
            1u128 << KAMINO_SF_SHIFT,
        );
        put_u128(&mut data, RES_PENDING_REFERRER_FEES_SF_OFF, 0);
        put_u64(&mut data, RES_COLLATERAL_MINT_TOTAL_SUPPLY_OFF, 800_000);

        let (total_supply, mint) =
            read_kamino_reserve_total_supply_and_mint_supply(&data).unwrap();
        // floor(1_000_000 + 5.5 - 2 - 1 - 0) = 1_000_002
        assert_eq!(total_supply, 1_000_002);
        assert_eq!(mint, 800_000);
    }

    #[test]
    fn reserve_decoder_rejects_bad_discriminator_and_short_data() {
        let mut data = vec![0u8; KAMINO_RESERVE_MIN_LEN];
        assert!(err_msg(
            read_kamino_reserve_total_supply_and_mint_supply(&data).unwrap_err()
        )
        .contains("discriminator"));
        data[..8].copy_from_slice(&KAMINO_RESERVE_DISCRIMINATOR);
        let short = &data[..KAMINO_RESERVE_MIN_LEN - 1];
        assert!(err_msg(
            read_kamino_reserve_total_supply_and_mint_supply(short).unwrap_err()
        )
        .contains("shorter"));
    }

    fn build_obligation(owner: &Pubkey, slots: &[(Pubkey, u64)]) -> Vec<u8> {
        let mut data = vec![0u8; KAMINO_OBLIGATION_MIN_LEN];
        data[..8].copy_from_slice(&KAMINO_OBLIGATION_DISCRIMINATOR);
        put_pubkey(&mut data, OBL_OWNER_OFF, owner);
        for (i, (reserve, amount)) in slots.iter().enumerate() {
            let slot = OBL_DEPOSITS_START + i * OBL_DEPOSIT_SLOT_SIZE;
            put_pubkey(&mut data, slot + OBL_DEPOSIT_RESERVE_OFF, reserve);
            put_u64(&mut data, slot + OBL_DEPOSIT_AMOUNT_OFF, *amount);
        }
        data
    }

    #[test]
    fn obligation_scanner_finds_correct_slot() {
        let owner = Pubkey::new_unique();
        let other = Pubkey::new_unique();
        let target = KAMINO_USDC_RESERVE;
        // Target sits in slot 3, behind an unrelated reserve in slot 0.
        let data = build_obligation(
            &owner,
            &[
                (other, 111),
                (Pubkey::default(), 0),
                (Pubkey::default(), 0),
                (target, 424_242),
            ],
        );
        let amount = read_kamino_obligation_collateral(&data, owner, target).unwrap();
        assert_eq!(amount, 424_242);
    }

    #[test]
    fn obligation_scanner_rejects_owner_mismatch() {
        let owner = Pubkey::new_unique();
        let data = build_obligation(&owner, &[(KAMINO_USDC_RESERVE, 5)]);
        let wrong = Pubkey::new_unique();
        assert!(err_msg(
            read_kamino_obligation_collateral(&data, wrong, KAMINO_USDC_RESERVE).unwrap_err()
        )
        .contains("owner"));
    }

    #[test]
    fn obligation_scanner_missing_reserve_errors() {
        let owner = Pubkey::new_unique();
        let data = build_obligation(&owner, &[(Pubkey::new_unique(), 5)]);
        assert!(err_msg(
            read_kamino_obligation_collateral(&data, owner, KAMINO_USDC_RESERVE).unwrap_err()
        )
        .contains("no deposit"));
    }

    #[test]
    fn collateral_to_assets_rounds_down_and_guards() {
        // 1000 cTokens * 2500 liquidity / 800 supply = 3125 (exact)
        assert_eq!(kamino_collateral_to_assets(1000, 2500, 800).unwrap(), 3125);
        // rounds down: 10 * 3 / 4 = 7.5 -> 7
        assert_eq!(kamino_collateral_to_assets(10, 3, 4).unwrap(), 7);
        // zero deposited -> zero assets (not an error)
        assert_eq!(kamino_collateral_to_assets(0, 2500, 800).unwrap(), 0);
        // zero supplies are guarded with distinct errors
        assert!(err_msg(kamino_collateral_to_assets(1, 2500, 0).unwrap_err())
            .contains("collateral mint total supply is zero"));
        assert!(err_msg(kamino_collateral_to_assets(1, 0, 800).unwrap_err())
            .contains("total liquidity supply is zero"));
        // multiplication overflow is checked
        assert!(kamino_collateral_to_assets(u64::MAX, u128::MAX, 1).is_err());
        // result exceeding u64 fails on the downcast
        assert!(kamino_collateral_to_assets(u64::MAX, u64::MAX as u128, 1).is_err());
    }
}

#[cfg(test)]
mod marginfi_decode_tests {
    use super::*;

    const ONE: u128 = 1u128 << 48; // I80F48 representation of 1.0

    fn err_msg(e: anchor_lang::error::Error) -> String {
        match e {
            anchor_lang::error::Error::AnchorError(ae) => ae.error_msg,
            _ => String::new(),
        }
    }
    fn put_pubkey(buf: &mut [u8], off: usize, k: &Pubkey) {
        buf[off..off + 32].copy_from_slice(k.as_ref());
    }
    fn put_i128(buf: &mut [u8], off: usize, v: i128) {
        buf[off..off + 16].copy_from_slice(&v.to_le_bytes());
    }

    fn build_bank(mint: &Pubkey, asset_share_value_raw: i128) -> Vec<u8> {
        let mut data = vec![0u8; MARGINFI_BANK_MIN_LEN];
        data[..8].copy_from_slice(&MARGINFI_BANK_DISCRIMINATOR);
        put_pubkey(&mut data, BANK_MINT_OFF, mint);
        put_i128(&mut data, BANK_ASSET_SHARE_VALUE_OFF, asset_share_value_raw);
        data
    }

    fn build_account(authority: &Pubkey, slots: &[(u8, Pubkey, i128)]) -> Vec<u8> {
        let mut data = vec![0u8; MARGINFI_ACCOUNT_MIN_LEN];
        data[..8].copy_from_slice(&MARGINFI_ACCOUNT_DISCRIMINATOR);
        put_pubkey(&mut data, MFA_AUTHORITY_OFF, authority);
        for (i, (active, bank_pk, shares_raw)) in slots.iter().enumerate() {
            let slot = MFA_BALANCES_OFF + i * MFA_BALANCE_SIZE;
            data[slot + BAL_ACTIVE_OFF] = *active;
            put_pubkey(&mut data, slot + BAL_BANK_PK_OFF, bank_pk);
            put_i128(&mut data, slot + BAL_ASSET_SHARES_OFF, *shares_raw);
        }
        data
    }

    #[test]
    fn bank_decoder_reads_asset_share_value() {
        let mint = Pubkey::new_unique();
        let data = build_bank(&mint, ONE as i128);
        assert_eq!(
            read_marginfi_bank_asset_share_value(&data, mint).unwrap(),
            ONE
        );
    }

    #[test]
    fn bank_decoder_rejects_bad_discriminator() {
        let mint = Pubkey::new_unique();
        let mut data = build_bank(&mint, ONE as i128);
        data[0] ^= 0xFF;
        assert!(err_msg(read_marginfi_bank_asset_share_value(&data, mint).unwrap_err())
            .contains("discriminator"));
    }

    #[test]
    fn bank_decoder_rejects_short_data_and_mint_mismatch() {
        let mint = Pubkey::new_unique();
        let data = build_bank(&mint, ONE as i128);
        let short = &data[..MARGINFI_BANK_MIN_LEN - 1];
        assert!(err_msg(read_marginfi_bank_asset_share_value(short, mint).unwrap_err())
            .contains("shorter"));
        let other = Pubkey::new_unique();
        assert!(err_msg(read_marginfi_bank_asset_share_value(&data, other).unwrap_err())
            .contains("mint"));
    }

    #[test]
    fn bank_decoder_rejects_negative_i80f48() {
        let mint = Pubkey::new_unique();
        let data = build_bank(&mint, -1i128);
        assert!(err_msg(read_marginfi_bank_asset_share_value(&data, mint).unwrap_err())
            .contains("negative"));
    }

    #[test]
    fn account_decoder_rejects_bad_discriminator() {
        let auth = Pubkey::new_unique();
        let bank = Pubkey::new_unique();
        let mut data = build_account(&auth, &[(1, bank, (1000u128 * ONE) as i128)]);
        data[1] ^= 0xFF;
        assert!(err_msg(read_marginfi_account_asset_shares(&data, auth, bank).unwrap_err())
            .contains("discriminator"));
    }

    #[test]
    fn account_decoder_rejects_authority_mismatch() {
        let auth = Pubkey::new_unique();
        let bank = Pubkey::new_unique();
        let data = build_account(&auth, &[(1, bank, (5u128 * ONE) as i128)]);
        let wrong = Pubkey::new_unique();
        assert!(err_msg(read_marginfi_account_asset_shares(&data, wrong, bank).unwrap_err())
            .contains("authority"));
    }

    #[test]
    fn account_decoder_finds_active_balance_and_skips_inactive() {
        let auth = Pubkey::new_unique();
        let usdc_bank = Pubkey::new_unique();
        // slot 0: inactive entry for the SAME bank (must be skipped);
        // slot 2: the real active balance.
        let data = build_account(
            &auth,
            &[
                (0, usdc_bank, (999u128 * ONE) as i128),
                (1, Pubkey::new_unique(), (7u128 * ONE) as i128),
                (1, usdc_bank, (555u128 * ONE) as i128),
            ],
        );
        let shares = read_marginfi_account_asset_shares(&data, auth, usdc_bank).unwrap();
        assert_eq!(shares, 555u128 * ONE);
    }

    #[test]
    fn account_decoder_no_active_usdc_balance_errors() {
        let auth = Pubkey::new_unique();
        let usdc_bank = Pubkey::new_unique();
        // usdc bank present but inactive; a different bank is active.
        let data = build_account(
            &auth,
            &[
                (0, usdc_bank, (10u128 * ONE) as i128),
                (1, Pubkey::new_unique(), (10u128 * ONE) as i128),
            ],
        );
        assert!(err_msg(
            read_marginfi_account_asset_shares(&data, auth, usdc_bank).unwrap_err()
        )
        .contains("no active"));
    }

    #[test]
    fn account_decoder_rejects_negative_shares() {
        let auth = Pubkey::new_unique();
        let bank = Pubkey::new_unique();
        let data = build_account(&auth, &[(1, bank, -5i128)]);
        assert!(err_msg(read_marginfi_account_asset_shares(&data, auth, bank).unwrap_err())
            .contains("negative"));
    }

    #[test]
    fn shares_to_assets_basic_and_zero() {
        // 1000 shares * 1.0 share value = 1000
        assert_eq!(marginfi_shares_to_assets(1000u128 * ONE, ONE).unwrap(), 1000);
        // zero shares -> zero assets (not an error)
        assert_eq!(marginfi_shares_to_assets(0, ONE).unwrap(), 0);
    }

    #[test]
    fn shares_to_assets_rounds_down() {
        // 7 shares * 1.5 share value = 10.5 -> 10
        let value_1_5 = ONE + (ONE / 2);
        assert_eq!(marginfi_shares_to_assets(7u128 * ONE, value_1_5).unwrap(), 10);
    }

    #[test]
    fn shares_to_assets_large_values_no_u128_overflow() {
        // 1e15 lamports of shares at 1.0 -> 1e15. The raw product is ~2^146,
        // which a naive u128 multiply cannot hold; the 256-bit path must.
        let big = 1_000_000_000_000_000u128;
        let shares_raw = big << 48;
        let value_raw = ONE;
        assert!(shares_raw.checked_mul(value_raw).is_none()); // proves naive u128 would overflow
        assert_eq!(u128::from(marginfi_shares_to_assets(shares_raw, value_raw).unwrap()), big);
    }

    #[test]
    fn shares_to_assets_result_exceeding_u64_errors() {
        // product = 2^160 -> result 2^64 which does not fit u64.
        assert!(marginfi_shares_to_assets(1u128 << 100, 1u128 << 60).is_err());
    }

    #[test]
    fn decode_pipeline_bank_and_account_to_assets() {
        let auth = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let usdc_bank = Pubkey::new_unique();
        let bank = build_bank(&mint, ONE as i128); // share value 1.0
        let acct = build_account(&auth, &[(1, usdc_bank, (1234u128 * ONE) as i128)]);
        let value_raw = read_marginfi_bank_asset_share_value(&bank, mint).unwrap();
        let shares_raw = read_marginfi_account_asset_shares(&acct, auth, usdc_bank).unwrap();
        assert_eq!(marginfi_shares_to_assets(shares_raw, value_raw).unwrap(), 1234);
    }

    #[test]
    fn current_value_pipeline_uses_pinned_usdc_bank() {
        let auth = Pubkey::new_unique();
        let value_1_5 = ONE + (ONE / 2);
        let bank = build_bank(&USDC_MINT, value_1_5 as i128);
        let acct = build_account(&auth, &[(1, MARGINFI_USDC_BANK, (7u128 * ONE) as i128)]);

        assert_eq!(marginfi_current_value_from_data(&bank, &acct, auth).unwrap(), 10);
    }

    #[test]
    fn current_value_pipeline_allows_zero_shares() {
        let auth = Pubkey::new_unique();
        let bank = build_bank(&USDC_MINT, ONE as i128);
        let acct = build_account(&auth, &[(1, MARGINFI_USDC_BANK, 0)]);

        assert_eq!(marginfi_current_value_from_data(&bank, &acct, auth).unwrap(), 0);
    }

    #[test]
    fn current_value_pipeline_rejects_wrong_bank_balance() {
        let auth = Pubkey::new_unique();
        let bank = build_bank(&USDC_MINT, ONE as i128);
        let acct = build_account(&auth, &[(1, Pubkey::new_unique(), (7u128 * ONE) as i128)]);

        assert!(err_msg(marginfi_current_value_from_data(&bank, &acct, auth).unwrap_err())
            .contains("no active"));
    }
}
