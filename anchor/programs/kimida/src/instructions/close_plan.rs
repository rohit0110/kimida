use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::KimiDaError,
    state::{Plan, PlanStatus},
};

/// UC4 / REQ15. Rent + sweep_tx returned/deleted via `close = owner`.
#[derive(Accounts)]
pub struct ClosePlan<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        seeds = [PLAN_SEED, owner.key().as_ref()],
        bump = plan.bump,
        has_one = owner,
    )]
    pub plan: Account<'info, Plan>,
}

pub fn handle_close_plan(ctx: Context<ClosePlan>) -> Result<()> {
    // REQ15: Active or Distributed may be closed. Keeper burns the nonce off-chain (REQ16).
    require!(
        matches!(ctx.accounts.plan.status, PlanStatus::Active | PlanStatus::Distributed),
        KimiDaError::InvalidStatus
    );
    Ok(())
}
