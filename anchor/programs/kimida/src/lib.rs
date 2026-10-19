pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("Egn9Kgqa8sLT2F5u5DS26Bg791eW51HfKKuqSSp8euke");

#[program]
pub mod kimida {
    use super::*;

    /// UC1 / REQ01
    pub fn initialize_plan(
        ctx: Context<InitializePlan>,
        beneficiary: Pubkey,
        keeper: Pubkey,
        interval: i64,
        grace: i64,
        sweep_tx: Vec<u8>,
    ) -> Result<()> {
        instructions::initialize_plan::handle_initialize_plan(
            ctx, beneficiary, keeper, interval, grace, sweep_tx,
        )
    }

    /// UC2 / REQ02
    pub fn check_in(ctx: Context<CheckIn>, sweep_tx: Vec<u8>) -> Result<()> {
        instructions::check_in::handle_check_in(ctx, sweep_tx)
    }

    /// UC3 / REQ03
    pub fn heir_sweep(ctx: Context<HeirSweep>) -> Result<()> {
        instructions::heir_sweep::handle_heir_sweep(ctx)
    }

    /// UC4 / REQ12
    pub fn close_plan(ctx: Context<ClosePlan>) -> Result<()> {
        instructions::close_plan::handle_close_plan(ctx)
    }

    /// UC5 / REQ13
    pub fn update_plan(
        ctx: Context<UpdatePlan>,
        interval: i64,
        grace: i64,
        sweep_tx: Vec<u8>,
    ) -> Result<()> {
        instructions::update_plan::handle_update_plan(ctx, interval, grace, sweep_tx)
    }
}
