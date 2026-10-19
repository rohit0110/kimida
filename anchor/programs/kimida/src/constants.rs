use anchor_lang::prelude::*;

#[constant]
pub const PLAN_SEED: &[u8] = b"plan";

/// Max size of a serialized transaction on Solana (REQ04).
// Not #[constant]: usize is unsupported in the IDL.
pub const MAX_SWEEP_TX_LEN: usize = 1232;

// TODO: confirm bounds (REQ01 "within bounds").
// `short-timers` is for localnet demos only: seconds-scale intervals.
#[cfg(not(feature = "short-timers"))]
mod bounds {
    pub const MIN_CHECKIN_INTERVAL: i64 = 24 * 60 * 60;
    pub const MIN_GRACE_PERIOD: i64 = 60 * 60;
}
#[cfg(feature = "short-timers")]
mod bounds {
    pub const MIN_CHECKIN_INTERVAL: i64 = 5;
    pub const MIN_GRACE_PERIOD: i64 = 1;
}

#[constant]
pub const MIN_CHECKIN_INTERVAL: i64 = bounds::MIN_CHECKIN_INTERVAL;
#[constant]
pub const MAX_CHECKIN_INTERVAL: i64 = 5 * 365 * 24 * 60 * 60;
#[constant]
pub const MIN_GRACE_PERIOD: i64 = bounds::MIN_GRACE_PERIOD;
#[constant]
pub const MAX_GRACE_PERIOD: i64 = 365 * 24 * 60 * 60;
