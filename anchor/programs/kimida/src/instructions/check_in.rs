use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::KimiDaError,
    state::{Plan, PlanStatus},
};

/// UC2 / REQ02, REQ10.
#[derive(Accounts)]
pub struct CheckIn<'info> {
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [PLAN_SEED, owner.key().as_ref()],
        bump = plan.bump,
        has_one = owner,
    )]
    pub plan: Account<'info, Plan>,
}

pub fn handle_check_in(ctx: Context<CheckIn>, sweep_tx: Vec<u8>) -> Result<()> {
    let plan = &mut ctx.accounts.plan;
    require!(plan.status == PlanStatus::Active, KimiDaError::PlanNotActive);
    Plan::validate_sweep_tx(&sweep_tx)?;
    plan.last_checkin = Clock::get()?.unix_timestamp;
    plan.sweep_tx = sweep_tx;
    Ok(())
}
