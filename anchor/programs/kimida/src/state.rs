use anchor_lang::prelude::*;

use crate::constants::MAX_SWEEP_TX_LEN;

/// REQ04. Seeds: ["plan", owner]. One plan per owner.
#[account]
#[derive(InitSpace)]
pub struct Plan {
    pub owner: Pubkey,
    pub beneficiary: Pubkey,
    pub keeper: Pubkey,
    pub nonce_account: Pubkey,
    /// Seconds between required check-ins.
    pub checkin_interval: i64,
    /// Extra seconds after the interval before the sweep may run.
    pub grace_period: i64,
    /// Unix timestamp of the last check-in / update.
    pub last_checkin: i64,
    pub status: PlanStatus,
    pub bump: u8,
    /// Owner-signed, keeper-unsigned `[AdvanceNonceAccount, heir_sweep]` (REQ10).
    #[max_len(MAX_SWEEP_TX_LEN)]
    pub sweep_tx: Vec<u8>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum PlanStatus {
    Active,
    Distributed,
}

impl Plan {
    /// REQ06: `last_checkin + interval + grace`.
    pub fn deadline(&self) -> i64 {
        self.last_checkin
            .saturating_add(self.checkin_interval)
            .saturating_add(self.grace_period)
    }

    pub fn validate_schedule(interval: i64, grace: i64) -> Result<()> {
        use crate::{constants::*, error::KimiDaError};
        require!(
            (MIN_CHECKIN_INTERVAL..=MAX_CHECKIN_INTERVAL).contains(&interval)
                && (MIN_GRACE_PERIOD..=MAX_GRACE_PERIOD).contains(&grace),
            KimiDaError::InvalidSchedule
        );
        Ok(())
    }

    pub fn validate_sweep_tx(sweep_tx: &[u8]) -> Result<()> {
        require!(
            sweep_tx.len() <= MAX_SWEEP_TX_LEN,
            crate::error::KimiDaError::SweepTxTooLarge
        );
        Ok(())
    }
}
