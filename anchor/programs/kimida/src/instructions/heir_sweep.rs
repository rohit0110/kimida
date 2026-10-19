use anchor_lang::prelude::*;

use solana_instructions_sysvar::load_instruction_at_checked;

use crate::{
    constants::*,
    error::KimiDaError,
    state::{Plan, PlanStatus},
};

const INSTRUCTIONS_SYSVAR: Pubkey = pubkey!("Sysvar1nstructions1111111111111111111111111");

/// UC3 / REQ03, REQ06-REQ09. Must be preceded by `AdvanceNonceAccount` at ix 0 (A1).
#[derive(Accounts)]
pub struct HeirSweep<'info> {
    /// Presigned earlier (offline).
    #[account(mut)]
    pub owner: Signer<'info>,
    /// Signs and pays at execution (REQ07).
    pub keeper: Signer<'info>,
    /// CHECK: checked against `plan.beneficiary` (REQ07).
    #[account(mut, address = plan.beneficiary @ KimiDaError::BeneficiaryMismatch)]
    pub beneficiary: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [PLAN_SEED, owner.key().as_ref()],
        bump = plan.bump,
        has_one = owner @ KimiDaError::BeneficiaryMismatch,
        has_one = keeper @ KimiDaError::KeeperOrNonceMismatch,
    )]
    pub plan: Account<'info, Plan>,
    /// CHECK: checked against `plan.nonce_account` (REQ07).
    #[account(address = plan.nonce_account @ KimiDaError::KeeperOrNonceMismatch)]
    pub nonce_account: UncheckedAccount<'info>,
    /// CHECK: instructions sysvar, used for the ix 0 check (A1).
    #[account(address = INSTRUCTIONS_SYSVAR)]
    pub instructions: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// System Program instruction index of `AdvanceNonceAccount`.
const ADVANCE_NONCE_IX: u32 = 4;

/// A1: instruction 0 must be System `AdvanceNonceAccount` on `plan.nonce_account`.
fn require_advance_nonce_first(ctx: &Context<HeirSweep>) -> Result<()> {
    let ix = load_instruction_at_checked(0, &ctx.accounts.instructions.to_account_info())
        .map_err(|_| error!(KimiDaError::MissingNonceAdvance))?;
    let is_advance = ix.program_id == anchor_lang::system_program::ID
        && ix.data.len() >= 4
        && u32::from_le_bytes(ix.data[0..4].try_into().unwrap()) == ADVANCE_NONCE_IX
        && ix.accounts.first().map(|a| a.pubkey) == Some(ctx.accounts.plan.nonce_account);
    require!(is_advance, KimiDaError::MissingNonceAdvance);
    Ok(())
}

pub fn handle_heir_sweep(ctx: Context<HeirSweep>) -> Result<()> {
    // REQ08: replay guard
    require!(
        ctx.accounts.plan.status == PlanStatus::Active,
        KimiDaError::AlreadyDistributed
    );
    // REQ06: deadline gate
    require!(
        Clock::get()?.unix_timestamp >= ctx.accounts.plan.deadline(),
        KimiDaError::DeadlineNotReached
    );
    // A1
    require_advance_nonce_first(&ctx)?;

    // REQ09: sweep the live balance
    ctx.accounts.plan.status = PlanStatus::Distributed;
    let amount = ctx.accounts.owner.lamports();
    let cpi = anchor_lang::system_program::Transfer {
        from: ctx.accounts.owner.to_account_info(),
        to: ctx.accounts.beneficiary.to_account_info(),
    };
    anchor_lang::system_program::transfer(
        CpiContext::new(anchor_lang::system_program::ID, cpi),
        amount,
    )
}
