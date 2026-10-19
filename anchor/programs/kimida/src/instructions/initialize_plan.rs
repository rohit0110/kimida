use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::KimiDaError,
    state::{Plan, PlanStatus},
};

/// UC1 / REQ01, REQ04, REQ05, REQ10.
#[derive(Accounts)]
pub struct InitializePlan<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        init,
        payer = owner,
        space = 8 + Plan::INIT_SPACE,
        seeds = [PLAN_SEED, owner.key().as_ref()],
        bump
    )]
    pub plan: Account<'info, Plan>,
    /// CHECK: System-owned nonce account, created client-side with authority = keeper.
    /// Authority is verified in the handler (REQ05).
    pub nonce_account: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// Reads the authority from a System Program nonce account.
/// Layout (bincode): u32 version | u32 state (1 = initialized) | authority (32) | ...
fn nonce_authority(nonce: &AccountInfo) -> Result<Pubkey> {
    require_keys_eq!(
        *nonce.owner,
        anchor_lang::system_program::ID,
        KimiDaError::InvalidNonceAccount
    );
    let data = nonce.try_borrow_data()?;
    require!(data.len() >= 40, KimiDaError::InvalidNonceAccount);
    let version = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let state = u32::from_le_bytes(data[4..8].try_into().unwrap());
    // Legacy (0) nonces cannot validate durable transactions; require Current (1) + Initialized (1).
    require!(version == 1 && state == 1, KimiDaError::InvalidNonceAccount);
    Ok(Pubkey::try_from(&data[8..40]).unwrap())
}

pub fn handle_initialize_plan(
    ctx: Context<InitializePlan>,
    beneficiary: Pubkey,
    keeper: Pubkey,
    interval: i64,
    grace: i64,
    sweep_tx: Vec<u8>,
) -> Result<()> {
    Plan::validate_schedule(interval, grace)?;
    Plan::validate_sweep_tx(&sweep_tx)?;
    require!(beneficiary != Pubkey::default(), KimiDaError::InvalidBeneficiary);

    let nonce = ctx.accounts.nonce_account.to_account_info();
    require_keys_eq!(
        nonce_authority(&nonce)?,
        keeper,
        KimiDaError::BadNonceAuthority
    );

    let plan = &mut ctx.accounts.plan;
    plan.owner = ctx.accounts.owner.key();
    plan.beneficiary = beneficiary;
    plan.keeper = keeper;
    plan.nonce_account = nonce.key();
    plan.checkin_interval = interval;
    plan.grace_period = grace;
    plan.last_checkin = Clock::get()?.unix_timestamp;
    plan.status = PlanStatus::Active;
    plan.bump = ctx.bumps.plan;
    plan.sweep_tx = sweep_tx;
    Ok(())
}
