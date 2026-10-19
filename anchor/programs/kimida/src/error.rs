use anchor_lang::prelude::*;

/// Names follow the "Custom program constraints" table in the Deliverable 2 PDF (section 5.4).
#[error_code]
pub enum KimiDaError {
    /// REQ02, REQ18
    #[msg("Check-in interval or grace period is zero or out of bounds")]
    InvalidSchedule,
    /// REQ03
    #[msg("Nonce account authority is not the keeper")]
    BadNonceAuthority,
    /// REQ04, REQ07, REQ20
    #[msg("sweep_tx exceeds the allocated space")]
    SweepTxTooLarge,
    /// REQ05, REQ17
    #[msg("Plan is not Active")]
    PlanNotActive,
    /// REQ15
    #[msg("Plan status does not allow this operation")]
    InvalidStatus,
    /// REQ11
    #[msg("Instruction 0 must be AdvanceNonceAccount on plan.nonce_account")]
    MissingNonceAdvance,
    /// REQ08
    #[msg("Deadline (last_checkin + interval + grace) not reached")]
    DeadlineNotReached,
    /// REQ09 (beneficiary or owner account differs from the plan)
    #[msg("Beneficiary or owner does not match plan")]
    BeneficiaryMismatch,
    /// REQ10
    #[msg("Keeper or nonce account does not match plan")]
    KeeperOrNonceMismatch,
    /// REQ12
    #[msg("Plan already distributed")]
    AlreadyDistributed,
    // Not in the PDF table: extra input hygiene.
    #[msg("Beneficiary must be a non-default pubkey")]
    InvalidBeneficiary,
    #[msg("Nonce account is not an initialized system nonce account")]
    InvalidNonceAccount,
}
