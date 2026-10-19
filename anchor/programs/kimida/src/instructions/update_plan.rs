use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::KimiDaError,
    state::{Plan, PlanStatus},
};

/// UC5 / REQ13.
#[derive(Accounts)]
pub struct UpdatePlan<'info> {
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [PLAN_SEED, owner.key().as_ref()],
        bump = plan.bump,
        has_one = owner,
    )]
    pub plan: Account<'info, Plan>,
}

pub fn handle_update_plan(
    ctx: Context<UpdatePlan>,
    interval: i64,
    grace: i64,
    sweep_tx: Vec<u8>,
) -> Result<()> {
    let plan = &mut ctx.accounts.plan;
    require!(plan.status == PlanStatus::Active, KimiDaError::PlanNotActive);
    Plan::validate_schedule(interval, grace)?;
    Plan::validate_sweep_tx(&sweep_tx)?;
    plan.checkin_interval = interval;
    plan.grace_period = grace;
    plan.last_checkin = Clock::get()?.unix_timestamp;
    plan.sweep_tx = sweep_tx;
    Ok(())
}
