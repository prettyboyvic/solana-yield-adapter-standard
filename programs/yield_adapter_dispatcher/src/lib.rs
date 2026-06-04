#![allow(deprecated, unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke,
};

declare_id!("CP1io1rppTn7HDg8qzpGh629KG6K23ipcgtVZjWMtXmw");

pub const REGISTRY_SEED: &[u8] = b"registry";
pub const REGISTRY_VERSION: u16 = 1;
pub const MAX_REGISTRY_URI_LEN: usize = 128;
pub const MAX_ADAPTER_URI_LEN: usize = 96;
pub const MAX_RISK_TIER: u8 = 5;
pub const CAPABILITY_DEPOSIT: u16 = 1 << 0;
pub const CAPABILITY_WITHDRAW: u16 = 1 << 1;
pub const CAPABILITY_CURRENT_VALUE: u16 = 1 << 2;

#[program]
pub mod yield_adapter_dispatcher {
    use super::*;

    pub fn initialize_registry(
        ctx: Context<InitializeRegistry>,
        max_adapters: u16,
        metadata_uri: String,
    ) -> Result<()> {
        require!(max_adapters > 0, YieldError::InvalidMaxAdapters);
        require!(
            metadata_uri.len() <= MAX_REGISTRY_URI_LEN,
            YieldError::MetadataUriTooLong
        );

        let registry = &mut ctx.accounts.registry;
        registry.governance = ctx.accounts.governance.key();
        registry.bump = ctx.bumps.registry;
        registry.version = REGISTRY_VERSION;
        registry.adapter_count = 0;
        registry.max_adapters = max_adapters;
        registry.metadata_uri = metadata_uri;
        registry.adapters = Vec::with_capacity(max_adapters as usize);

        Ok(())
    }

    pub fn set_governance(ctx: Context<GovernanceOnly>, new_governance: Pubkey) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.governance.key(),
            ctx.accounts.registry.governance,
            YieldError::Unauthorized
        );
        require_keys_neq!(
            new_governance,
            Pubkey::default(),
            YieldError::InvalidGovernance
        );

        ctx.accounts.registry.governance = new_governance;
        emit!(GovernanceChanged { new_governance });
        Ok(())
    }

    pub fn register_adapter(
        ctx: Context<RegisterAdapter>,
        adapter_id: [u8; 32],
        config: AdapterRegistration,
    ) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        require_keys_eq!(
            ctx.accounts.governance.key(),
            registry.governance,
            YieldError::Unauthorized
        );
        require!(
            registry.adapter_count < registry.max_adapters,
            YieldError::RegistryFull
        );
        require!(
            config.metadata_uri.len() <= MAX_ADAPTER_URI_LEN,
            YieldError::MetadataUriTooLong
        );
        require!(config.risk_tier <= MAX_RISK_TIER, YieldError::InvalidRiskTier);
        require!(
            config.capabilities & (CAPABILITY_DEPOSIT | CAPABILITY_WITHDRAW | CAPABILITY_CURRENT_VALUE) != 0,
            YieldError::InvalidCapabilities
        );
        require!(
            registry.find_adapter(adapter_id).is_none(),
            YieldError::AdapterAlreadyRegistered
        );

        let record = AdapterRecord {
            adapter_id,
            program_id: ctx.accounts.adapter_program.key(),
            protocol: config.protocol,
            underlying_mint: config.underlying_mint,
            receipt_mint: config.receipt_mint,
            authority: config.authority,
            capabilities: config.capabilities,
            risk_tier: config.risk_tier,
            active: true,
            metadata_uri: config.metadata_uri,
            metadata_hash: config.metadata_hash,
        };

        registry.adapters.push(record.clone());
        registry.adapter_count = registry
            .adapter_count
            .checked_add(1)
            .ok_or(YieldError::MathOverflow)?;

        emit!(AdapterRegistered {
            adapter_id,
            program_id: record.program_id,
            protocol: record.protocol,
            underlying_mint: record.underlying_mint,
            receipt_mint: record.receipt_mint,
        });

        Ok(())
    }

    pub fn set_adapter_status(
        ctx: Context<GovernanceOnly>,
        adapter_id: [u8; 32],
        active: bool,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.governance.key(),
            ctx.accounts.registry.governance,
            YieldError::Unauthorized
        );

        let record = ctx
            .accounts
            .registry
            .find_adapter_mut(adapter_id)
            .ok_or(YieldError::AdapterNotFound)?;
        record.active = active;

        emit!(AdapterStatusSet { adapter_id, active });
        Ok(())
    }

    pub fn deposit<'info>(
        ctx: Context<'_, '_, '_, 'info, Route<'info>>,
        adapter_id: [u8; 32],
        amount: u64,
        min_shares_out: u64,
    ) -> Result<()> {
        let payload = DepositArgs {
            adapter_id,
            amount,
            min_shares_out,
        };
        route_to_adapter(
            &ctx.accounts.registry,
            &ctx.accounts.adapter_program,
            &ctx.accounts.user,
            ctx.remaining_accounts,
            adapter_id,
            AdapterAction::Deposit,
            "deposit",
            &payload,
        )
    }

    pub fn withdraw<'info>(
        ctx: Context<'_, '_, '_, 'info, Route<'info>>,
        adapter_id: [u8; 32],
        shares: u64,
        min_assets_out: u64,
    ) -> Result<()> {
        let payload = WithdrawArgs {
            adapter_id,
            shares,
            min_assets_out,
        };
        route_to_adapter(
            &ctx.accounts.registry,
            &ctx.accounts.adapter_program,
            &ctx.accounts.user,
            ctx.remaining_accounts,
            adapter_id,
            AdapterAction::Withdraw,
            "withdraw",
            &payload,
        )
    }

    pub fn current_value<'info>(
        ctx: Context<'_, '_, '_, 'info, Route<'info>>,
        adapter_id: [u8; 32],
    ) -> Result<()> {
        let payload = CurrentValueArgs { adapter_id };
        route_to_adapter(
            &ctx.accounts.registry,
            &ctx.accounts.adapter_program,
            &ctx.accounts.user,
            ctx.remaining_accounts,
            adapter_id,
            AdapterAction::CurrentValue,
            "current_value",
            &payload,
        )
    }
}

fn route_to_adapter<'info, P: AnchorSerialize>(
    registry: &Account<'info, Registry>,
    adapter_program: &UncheckedAccount<'info>,
    user: &Signer<'info>,
    remaining_accounts: &[AccountInfo<'info>],
    adapter_id: [u8; 32],
    action: AdapterAction,
    anchor_method: &str,
    payload: &P,
) -> Result<()> {
    let record = registry
        .find_adapter(adapter_id)
        .ok_or(YieldError::AdapterNotFound)?;
    require!(record.active, YieldError::AdapterInactive);
    require_keys_eq!(
        adapter_program.key(),
        record.program_id,
        YieldError::AdapterProgramMismatch
    );
    require!(
        record.capabilities & action.capability() != 0,
        YieldError::UnsupportedAction
    );

    let mut data = anchor_discriminator(anchor_method).to_vec();
    payload
        .serialize(&mut data)
        .map_err(|_| error!(YieldError::SerializationFailed))?;

    let mut metas = Vec::with_capacity(1 + remaining_accounts.len());
    metas.push(AccountMeta::new_readonly(user.key(), true));
    for account in remaining_accounts {
        let key = account.key();
        if account.is_writable {
            metas.push(AccountMeta::new(key, account.is_signer));
        } else {
            metas.push(AccountMeta::new_readonly(key, account.is_signer));
        }
    }

    let instruction = Instruction {
        program_id: record.program_id,
        accounts: metas,
        data,
    };

    let mut infos = Vec::with_capacity(2 + remaining_accounts.len());
    infos.push(adapter_program.to_account_info());
    infos.push(user.to_account_info());
    infos.extend(remaining_accounts.iter().cloned());

    invoke(&instruction, &infos)?;

    emit!(Routed {
        adapter_id,
        program_id: record.program_id,
        action: action as u8,
        user: user.key(),
    });

    Ok(())
}

fn anchor_discriminator(method: &str) -> [u8; 8] {
    let preimage = format!("global:{method}");
    let digest = hash(preimage.as_bytes()).to_bytes();
    let mut discriminator = [0_u8; 8];
    discriminator.copy_from_slice(&digest[..8]);
    discriminator
}

#[derive(Accounts)]
#[instruction(max_adapters: u16, metadata_uri: String)]
pub struct InitializeRegistry<'info> {
    #[account(
        init,
        payer = payer,
        space = Registry::space(max_adapters as usize),
        seeds = [REGISTRY_SEED],
        bump
    )]
    pub registry: Account<'info, Registry>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub governance: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GovernanceOnly<'info> {
    #[account(mut, seeds = [REGISTRY_SEED], bump = registry.bump)]
    pub registry: Account<'info, Registry>,
    pub governance: Signer<'info>,
}

#[derive(Accounts)]
pub struct RegisterAdapter<'info> {
    #[account(mut, seeds = [REGISTRY_SEED], bump = registry.bump)]
    pub registry: Account<'info, Registry>,
    pub governance: Signer<'info>,
    /// CHECK: The registry stores and later validates this executable program id.
    pub adapter_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Route<'info> {
    #[account(seeds = [REGISTRY_SEED], bump = registry.bump)]
    pub registry: Account<'info, Registry>,
    /// CHECK: Must match the registered adapter program id.
    pub adapter_program: UncheckedAccount<'info>,
    pub user: Signer<'info>,
}

#[account]
pub struct Registry {
    pub governance: Pubkey,
    pub bump: u8,
    pub version: u16,
    pub adapter_count: u16,
    pub max_adapters: u16,
    pub metadata_uri: String,
    pub adapters: Vec<AdapterRecord>,
}

impl Registry {
    pub const BASE_SPACE: usize = 32 + 1 + 2 + 2 + 2 + (4 + MAX_REGISTRY_URI_LEN) + 4;

    pub fn space(max_adapters: usize) -> usize {
        8 + Self::BASE_SPACE + max_adapters * AdapterRecord::SPACE
    }

    pub fn find_adapter(&self, adapter_id: [u8; 32]) -> Option<&AdapterRecord> {
        self.adapters
            .iter()
            .find(|adapter| adapter.adapter_id == adapter_id)
    }

    pub fn find_adapter_mut(&mut self, adapter_id: [u8; 32]) -> Option<&mut AdapterRecord> {
        self.adapters
            .iter_mut()
            .find(|adapter| adapter.adapter_id == adapter_id)
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdapterRecord {
    pub adapter_id: [u8; 32],
    pub program_id: Pubkey,
    pub protocol: u8,
    pub underlying_mint: Pubkey,
    pub receipt_mint: Pubkey,
    pub authority: Pubkey,
    pub capabilities: u16,
    pub risk_tier: u8,
    pub active: bool,
    pub metadata_uri: String,
    pub metadata_hash: [u8; 32],
}

impl AdapterRecord {
    pub const SPACE: usize = 32 + 32 + 1 + 32 + 32 + 32 + 2 + 1 + 1 + (4 + MAX_ADAPTER_URI_LEN) + 32;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AdapterRegistration {
    pub protocol: u8,
    pub underlying_mint: Pubkey,
    pub receipt_mint: Pubkey,
    pub authority: Pubkey,
    pub capabilities: u16,
    pub risk_tier: u8,
    pub metadata_uri: String,
    pub metadata_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct DepositArgs {
    pub adapter_id: [u8; 32],
    pub amount: u64,
    pub min_shares_out: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawArgs {
    pub adapter_id: [u8; 32],
    pub shares: u64,
    pub min_assets_out: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CurrentValueArgs {
    pub adapter_id: [u8; 32],
}

#[derive(Clone, Copy)]
pub enum AdapterAction {
    Deposit = 1,
    Withdraw = 2,
    CurrentValue = 3,
}

impl AdapterAction {
    pub fn capability(self) -> u16 {
        match self {
            AdapterAction::Deposit => CAPABILITY_DEPOSIT,
            AdapterAction::Withdraw => CAPABILITY_WITHDRAW,
            AdapterAction::CurrentValue => CAPABILITY_CURRENT_VALUE,
        }
    }
}

#[event]
pub struct AdapterRegistered {
    pub adapter_id: [u8; 32],
    pub program_id: Pubkey,
    pub protocol: u8,
    pub underlying_mint: Pubkey,
    pub receipt_mint: Pubkey,
}

#[event]
pub struct AdapterStatusSet {
    pub adapter_id: [u8; 32],
    pub active: bool,
}

#[event]
pub struct GovernanceChanged {
    pub new_governance: Pubkey,
}

#[event]
pub struct Routed {
    pub adapter_id: [u8; 32],
    pub program_id: Pubkey,
    pub action: u8,
    pub user: Pubkey,
}

#[error_code]
pub enum YieldError {
    #[msg("registry max adapter count must be greater than zero")]
    InvalidMaxAdapters,
    #[msg("metadata URI exceeds the configured maximum length")]
    MetadataUriTooLong,
    #[msg("caller is not registry governance")]
    Unauthorized,
    #[msg("new governance cannot be the default pubkey")]
    InvalidGovernance,
    #[msg("registry has reached its adapter limit")]
    RegistryFull,
    #[msg("adapter is already registered")]
    AdapterAlreadyRegistered,
    #[msg("adapter was not found in the registry")]
    AdapterNotFound,
    #[msg("adapter is inactive")]
    AdapterInactive,
    #[msg("provided adapter program does not match the registry")]
    AdapterProgramMismatch,
    #[msg("adapter does not support this action")]
    UnsupportedAction,
    #[msg("risk tier is out of range")]
    InvalidRiskTier,
    #[msg("capability mask is invalid")]
    InvalidCapabilities,
    #[msg("serialization failed")]
    SerializationFailed,
    #[msg("math overflow")]
    MathOverflow,
}
