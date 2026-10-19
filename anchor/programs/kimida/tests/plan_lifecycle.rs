//! One test (or more) per requirement. Needs `anchor build` first (loads target/deploy/kimida.so).
//! Requirement ids follow the Deliverable 2 PDF (REQ01-REQ26). Off-chain requirements
//! (REQ16, REQ21-REQ24, REQ26) are covered in keeper/tests/e2e_multi_user.rs.

use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    kimida::{PlanStatus, MAX_SWEEP_TX_LEN, PLAN_SEED},
    litesvm::LiteSVM,
    solana_account::Account,
    solana_clock::Clock,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const DAY: i64 = 24 * 60 * 60;
const INTERVAL: i64 = 30 * DAY;
const GRACE: i64 = 7 * DAY;
const T0: i64 = 1_000_000;
const SOL: u64 = 1_000_000_000;

struct Env {
    svm: LiteSVM,
    owner: Keypair,
    keeper: Keypair,
    beneficiary: Pubkey,
    nonce: Pubkey,
    plan: Pubkey,
}

fn set_time(svm: &mut LiteSVM, t: i64) {
    let mut clock: Clock = svm.get_sysvar();
    clock.unix_timestamp = t;
    svm.set_sysvar(&clock);
}

/// Bincode layout of an initialized System nonce account (Versions::Current).
fn nonce_account(authority: &Pubkey) -> Account {
    let mut data = Vec::with_capacity(80);
    data.extend_from_slice(&1u32.to_le_bytes()); // Versions::Current
    data.extend_from_slice(&1u32.to_le_bytes()); // State::Initialized
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&[7u8; 32]); // stored durable nonce (!= current blockhash)
    data.extend_from_slice(&5000u64.to_le_bytes()); // lamports_per_signature
    Account {
        lamports: 2_000_000,
        data,
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn new_env() -> Env {
    let program_id = kimida::id();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/kimida.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    set_time(&mut svm, T0);

    let owner = Keypair::new();
    let keeper = Keypair::new();
    let nonce = Pubkey::new_unique();
    svm.airdrop(&owner.pubkey(), 10 * SOL).unwrap();
    svm.airdrop(&keeper.pubkey(), 10 * SOL).unwrap();
    svm.set_account(nonce, nonce_account(&keeper.pubkey())).unwrap();
    let plan = Pubkey::find_program_address(&[PLAN_SEED, owner.pubkey().as_ref()], &program_id).0;
    Env {
        svm,
        owner,
        keeper,
        beneficiary: Pubkey::new_unique(),
        nonce,
        plan,
    }
}

fn sweep_bytes() -> Vec<u8> {
    vec![1u8; 300]
}

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    extra: &[&Keypair],
) -> Result<(), String> {
    svm.expire_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &svm.latest_blockhash());
    let mut signers = vec![payer];
    signers.extend_from_slice(extra);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?} {}", e.err, e.meta.logs.join("\n")))
}

// ---- instruction builders ----

fn ix_init(e: &Env, ben: Pubkey, keeper: Pubkey, nonce: Pubkey, interval: i64, grace: i64, tx: Vec<u8>) -> Instruction {
    Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::InitializePlan {
            beneficiary: ben,
            keeper,
            interval,
            grace,
            sweep_tx: tx,
        }
        .data(),
        kimida::accounts::InitializePlan {
            owner: e.owner.pubkey(),
            plan: e.plan,
            nonce_account: nonce,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

fn ix_check_in(_e: &Env, signer: Pubkey, plan: Pubkey, tx: Vec<u8>) -> Instruction {
    Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::CheckIn { sweep_tx: tx }.data(),
        kimida::accounts::CheckIn { owner: signer, plan }.to_account_metas(None),
    )
}

fn ix_update(signer: Pubkey, plan: Pubkey, interval: i64, grace: i64, tx: Vec<u8>) -> Instruction {
    Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::UpdatePlan {
            interval,
            grace,
            sweep_tx: tx,
        }
        .data(),
        kimida::accounts::UpdatePlan { owner: signer, plan }.to_account_metas(None),
    )
}

fn ix_close(signer: Pubkey, plan: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::ClosePlan {}.data(),
        kimida::accounts::ClosePlan { owner: signer, plan }.to_account_metas(None),
    )
}

fn ix_advance(nonce: Pubkey, authority: Pubkey) -> Instruction {
    solana_system_interface::instruction::advance_nonce_account(&nonce, &authority)
}

fn ix_sweep(e: &Env, beneficiary: Pubkey, keeper: Pubkey, nonce: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::HeirSweep {}.data(),
        kimida::accounts::HeirSweep {
            owner: e.owner.pubkey(),
            keeper,
            beneficiary,
            plan: e.plan,
            nonce_account: nonce,
            instructions: solana_sdk_ids::sysvar::instructions::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

// ---- helpers ----

fn init_ok(e: &mut Env) {
    let ix = ix_init(e, e.beneficiary, e.keeper.pubkey(), e.nonce, INTERVAL, GRACE, sweep_bytes());
    send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]).unwrap();
}

fn load_plan(e: &Env) -> kimida::Plan {
    let acc = e.svm.get_account(&e.plan).expect("plan account missing");
    kimida::Plan::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

fn sweep(e: &mut Env) -> Result<(), String> {
    let ixs = [
        ix_advance(e.nonce, e.keeper.pubkey()),
        ix_sweep(e, e.beneficiary, e.keeper.pubkey(), e.nonce),
    ];
    send(&mut e.svm, &ixs, &e.keeper.insecure_clone(), &[&e.owner.insecure_clone()])
}

fn deadline() -> i64 {
    T0 + INTERVAL + GRACE
}

fn assert_err(res: Result<(), String>, needle: &str) {
    let err = res.expect_err("expected failure");
    assert!(err.contains(needle), "expected `{needle}` in:\n{err}");
}

// ================= REQ01-REQ04, REQ25: initialize_plan =================

#[test]
fn req25_plan_holds_exact_fields_after_init() {
    let mut e = new_env();
    init_ok(&mut e);
    let p = load_plan(&e);
    assert_eq!(p.owner, e.owner.pubkey());
    assert_eq!(p.beneficiary, e.beneficiary);
    assert_eq!(p.keeper, e.keeper.pubkey());
    assert_eq!(p.nonce_account, e.nonce);
    assert_eq!(p.checkin_interval, INTERVAL);
    assert_eq!(p.grace_period, GRACE);
    assert_eq!(p.last_checkin, T0);
    assert_eq!(p.status, PlanStatus::Active);
}

#[test]
fn req02_rejects_bad_interval_and_grace() {
    let mut e = new_env();
    let owner = e.owner.insecure_clone();
    let ks = e.keeper.pubkey();
    for (ben, iv, gr, needle) in [
        (e.beneficiary, 0, GRACE, "InvalidSchedule"),
        (e.beneficiary, INTERVAL, 0, "InvalidSchedule"),
        (e.beneficiary, INTERVAL + 5 * 365 * DAY, GRACE, "InvalidSchedule"),
    ] {
        let ix = ix_init(&e, ben, ks, e.nonce, iv, gr, sweep_bytes());
        assert_err(send(&mut e.svm, &[ix], &owner, &[]), needle);
    }
}

#[test]
fn req01_cannot_initialize_twice() {
    let mut e = new_env();
    init_ok(&mut e);
    let ix = ix_init(&e, e.beneficiary, e.keeper.pubkey(), e.nonce, INTERVAL, GRACE, sweep_bytes());
    assert!(send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]).is_err());
}

// ================= REQ04 Plan layout / sweep_tx / seeds =================

#[test]
fn req01_req04_plan_is_program_owned_pda_and_stores_sweep_tx() {
    let mut e = new_env();
    init_ok(&mut e);
    let acc = e.svm.get_account(&e.plan).unwrap();
    assert_eq!(acc.owner, kimida::id());
    assert_eq!(load_plan(&e).sweep_tx, sweep_bytes());
    assert!(load_plan(&e).bump > 0);
}

#[test]
fn req04_sweep_tx_over_max_len_rejected() {
    let mut e = new_env();
    let ix = ix_init(&e, e.beneficiary, e.keeper.pubkey(), e.nonce, INTERVAL, GRACE, vec![0u8; MAX_SWEEP_TX_LEN + 1]);
    // Over-long args also exceed the 1232-byte tx limit; either failure is acceptable.
    let owner = e.owner.insecure_clone();
    assert!(send(&mut e.svm, &[ix], &owner, &[]).is_err());
}

// ================= REQ05 nonce account (on-chain part: authority == keeper) =================

#[test]
fn req03_nonce_authority_must_be_keeper() {
    let mut e = new_env();
    let other = Pubkey::new_unique();
    e.svm.set_account(other, nonce_account(&Pubkey::new_unique())).unwrap();
    let ix = ix_init(&e, e.beneficiary, e.keeper.pubkey(), other, INTERVAL, GRACE, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]), "BadNonceAuthority");
}

#[test]
fn extra_nonce_must_be_initialized_system_account() {
    let mut e = new_env();
    let junk = Pubkey::new_unique(); // doesn't exist
    let ix = ix_init(&e, e.beneficiary, e.keeper.pubkey(), junk, INTERVAL, GRACE, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]), "InvalidNonceAccount");
}

#[test]
fn extra_legacy_version_nonce_rejected() {
    let mut e = new_env();
    let legacy = Pubkey::new_unique();
    let mut acc = nonce_account(&e.keeper.pubkey());
    acc.data[0..4].copy_from_slice(&0u32.to_le_bytes()); // Versions::Legacy
    e.svm.set_account(legacy, acc).unwrap();
    let ix = ix_init(&e, e.beneficiary, e.keeper.pubkey(), legacy, INTERVAL, GRACE, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]), "InvalidNonceAccount");
}

// ================= REQ02 check_in =================

#[test]
fn req06_req07_check_in_sets_last_checkin_and_overwrites_sweep_tx() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, T0 + 5 * DAY);
    let ix = ix_check_in(&e, e.owner.pubkey(), e.plan, vec![9u8; 120]);
    send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]).unwrap();
    let p = load_plan(&e);
    assert_eq!(p.last_checkin, T0 + 5 * DAY);
    // REQ10 on-chain half: sweep_tx overwritten
    assert_eq!(p.sweep_tx, vec![9u8; 120]);
}

#[test]
fn req05_only_owner_can_check_in() {
    let mut e = new_env();
    init_ok(&mut e);
    let stranger = Keypair::new();
    e.svm.airdrop(&stranger.pubkey(), SOL).unwrap();
    // stranger signs, but supplies the owner's plan
    let ix = ix_check_in(&e, stranger.pubkey(), e.plan, sweep_bytes());
    assert!(send(&mut e.svm, &[ix], &stranger, &[]).is_err());
}

#[test]
fn req05_check_in_rejected_when_distributed() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    sweep(&mut e).unwrap();
    e.svm.airdrop(&e.owner.pubkey(), SOL).unwrap(); // owner was drained; needs fee funds
    let ix = ix_check_in(&e, e.owner.pubkey(), e.plan, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]), "PlanNotActive");
}

// ================= REQ03 heir_sweep =================

#[test]
fn req13_req14_sweep_moves_live_balance_and_sets_distributed() {
    let mut e = new_env();
    init_ok(&mut e);
    let owner_bal = e.svm.get_balance(&e.owner.pubkey()).unwrap();
    set_time(&mut e.svm, deadline());
    sweep(&mut e).unwrap();
    assert_eq!(e.svm.get_balance(&e.beneficiary).unwrap(), owner_bal);
    assert_eq!(e.svm.get_balance(&e.owner.pubkey()).unwrap_or(0), 0);
    assert_eq!(load_plan(&e).status, PlanStatus::Distributed);
}

#[test]
fn req13_sweep_uses_live_balance_at_execution() {
    let mut e = new_env();
    init_ok(&mut e);
    e.svm.airdrop(&e.owner.pubkey(), 3 * SOL).unwrap(); // balance grows after presign
    let owner_bal = e.svm.get_balance(&e.owner.pubkey()).unwrap();
    set_time(&mut e.svm, deadline());
    sweep(&mut e).unwrap();
    assert_eq!(e.svm.get_balance(&e.beneficiary).unwrap(), owner_bal);
}

// ================= REQ06 deadline gate =================

#[test]
fn req08_sweep_before_deadline_reverts() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline() - 1);
    assert_err(sweep(&mut e), "DeadlineNotReached");
    assert_eq!(load_plan(&e).status, PlanStatus::Active);
}

#[test]
fn req08_check_in_pushes_deadline_out() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, T0 + 10 * DAY);
    let ix = ix_check_in(&e, e.owner.pubkey(), e.plan, sweep_bytes());
    send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]).unwrap();
    set_time(&mut e.svm, deadline()); // old deadline, now too early
    assert_err(sweep(&mut e), "DeadlineNotReached");
    set_time(&mut e.svm, T0 + 10 * DAY + INTERVAL + GRACE);
    sweep(&mut e).unwrap();
}

// ================= REQ07 binding checks =================

#[test]
fn req09_wrong_beneficiary_reverts() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    let ixs = [
        ix_advance(e.nonce, e.keeper.pubkey()),
        ix_sweep(&e, Pubkey::new_unique(), e.keeper.pubkey(), e.nonce),
    ];
    assert_err(
        send(&mut e.svm, &ixs, &e.keeper.insecure_clone(), &[&e.owner.insecure_clone()]),
        "BeneficiaryMismatch",
    );
}

#[test]
fn req10_wrong_keeper_reverts() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    let imposter = Keypair::new();
    e.svm.airdrop(&imposter.pubkey(), SOL).unwrap();
    let ixs = [
        ix_advance(e.nonce, e.keeper.pubkey()),
        ix_sweep(&e, e.beneficiary, imposter.pubkey(), e.nonce),
    ];
    assert_err(
        send(&mut e.svm, &ixs, &imposter, &[&e.owner.insecure_clone(), &e.keeper.insecure_clone()]),
        "KeeperOrNonceMismatch",
    );
}

#[test]
fn req10_wrong_nonce_reverts() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    let other = Pubkey::new_unique();
    e.svm.set_account(other, nonce_account(&e.keeper.pubkey())).unwrap();
    let ixs = [
        ix_advance(other, e.keeper.pubkey()),
        ix_sweep(&e, e.beneficiary, e.keeper.pubkey(), other),
    ];
    assert_err(
        send(&mut e.svm, &ixs, &e.keeper.insecure_clone(), &[&e.owner.insecure_clone()]),
        "KeeperOrNonceMismatch",
    );
}

#[test]
fn req09_wrong_owner_reverts() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    // another wallet's plan PDA does not exist -> the plan seeds check fails
    let other_owner = Keypair::new();
    e.svm.airdrop(&other_owner.pubkey(), SOL).unwrap();
    let mut ix = ix_sweep(&e, e.beneficiary, e.keeper.pubkey(), e.nonce);
    ix.accounts[0].pubkey = other_owner.pubkey();
    let ixs = [ix_advance(e.nonce, e.keeper.pubkey()), ix];
    assert!(send(&mut e.svm, &ixs, &e.keeper.insecure_clone(), &[&other_owner]).is_err());
}

// ================= REQ08 replay guard =================

#[test]
fn req12_second_sweep_reverts() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    sweep(&mut e).unwrap();
    assert_err(sweep(&mut e), "AlreadyDistributed");
}

// ================= REQ09 transfer CPI (covered by REQ03; fee payer check) =================

#[test]
fn req13_keeper_pays_fee_owner_fully_drained() {
    let mut e = new_env();
    init_ok(&mut e);
    let keeper_before = e.svm.get_balance(&e.keeper.pubkey()).unwrap();
    set_time(&mut e.svm, deadline());
    sweep(&mut e).unwrap();
    assert!(e.svm.get_balance(&e.keeper.pubkey()).unwrap() < keeper_before);
    assert_eq!(e.svm.get_balance(&e.owner.pubkey()).unwrap_or(0), 0);
}

// ================= A1 instruction-0 check =================

#[test]
fn req11_sweep_without_advance_nonce_ix0_reverts() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    let ixs = [ix_sweep(&e, e.beneficiary, e.keeper.pubkey(), e.nonce)];
    assert_err(
        send(&mut e.svm, &ixs, &e.keeper.insecure_clone(), &[&e.owner.insecure_clone()]),
        "MissingNonceAdvance",
    );
}

// ================= REQ10 sweep_tx plumbing =================

#[test]
fn req07_req20_sweep_tx_overwritten_by_check_in_and_update() {
    let mut e = new_env();
    init_ok(&mut e);
    let owner = e.owner.insecure_clone();
    let ix = ix_check_in(&e, owner.pubkey(), e.plan, vec![2u8; 50]);
    send(&mut e.svm, &[ix], &owner, &[]).unwrap();
    assert_eq!(load_plan(&e).sweep_tx, vec![2u8; 50]);
    let ix = ix_update(owner.pubkey(), e.plan, INTERVAL, GRACE, vec![3u8; 60]);
    send(&mut e.svm, &[ix], &owner, &[]).unwrap();
    assert_eq!(load_plan(&e).sweep_tx, vec![3u8; 60]);
}

// ================= REQ11 keeper (off-chain) =================

// ================= REQ12 close_plan =================

#[test]
fn req15_close_returns_rent_and_deletes_plan() {
    let mut e = new_env();
    init_ok(&mut e);
    let owner = e.owner.insecure_clone();
    let before = e.svm.get_balance(&owner.pubkey()).unwrap();
    send(&mut e.svm, &[ix_close(owner.pubkey(), e.plan)], &owner, &[]).unwrap();
    assert!(e.svm.get_account(&e.plan).is_none());
    assert!(e.svm.get_balance(&owner.pubkey()).unwrap() > before - 100_000);
}

#[test]
fn req15_only_owner_can_close() {
    let mut e = new_env();
    init_ok(&mut e);
    let stranger = Keypair::new();
    e.svm.airdrop(&stranger.pubkey(), SOL).unwrap();
    assert!(send(&mut e.svm, &[ix_close(stranger.pubkey(), e.plan)], &stranger, &[]).is_err());
    assert!(e.svm.get_account(&e.plan).is_some());
}

#[test]
fn req15_close_allowed_after_distributed() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    sweep(&mut e).unwrap();
    // owner was drained; fund it again so it can pay the close fee
    e.svm.airdrop(&e.owner.pubkey(), SOL).unwrap();
    let owner = e.owner.insecure_clone();
    send(&mut e.svm, &[ix_close(owner.pubkey(), e.plan)], &owner, &[]).unwrap();
    assert!(e.svm.get_account(&e.plan).is_none());
}

#[test]
fn req15_can_reinitialize_after_close() {
    let mut e = new_env();
    init_ok(&mut e);
    let owner = e.owner.insecure_clone();
    send(&mut e.svm, &[ix_close(owner.pubkey(), e.plan)], &owner, &[]).unwrap();
    init_ok(&mut e);
    assert_eq!(load_plan(&e).status, PlanStatus::Active);
}

// ================= REQ13 update_plan =================

#[test]
fn req18_req19_req20_update_replaces_schedule_resets_timer_and_sweep_tx() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, T0 + 3 * DAY);
    let owner = e.owner.insecure_clone();
    let ix = ix_update(owner.pubkey(), e.plan, 60 * DAY, 10 * DAY, vec![4u8; 80]);
    send(&mut e.svm, &[ix], &owner, &[]).unwrap();
    let p = load_plan(&e);
    assert_eq!(p.checkin_interval, 60 * DAY);
    assert_eq!(p.grace_period, 10 * DAY);
    assert_eq!(p.last_checkin, T0 + 3 * DAY);
    assert_eq!(p.sweep_tx, vec![4u8; 80]);
}

#[test]
fn req18_update_validates_bounds() {
    let mut e = new_env();
    init_ok(&mut e);
    let owner = e.owner.insecure_clone();
    let ix = ix_update(owner.pubkey(), e.plan, 0, GRACE, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &owner, &[]), "InvalidSchedule");
    let ix = ix_update(owner.pubkey(), e.plan, INTERVAL, 0, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &owner, &[]), "InvalidSchedule");
}

#[test]
fn req17_update_rejected_when_distributed() {
    let mut e = new_env();
    init_ok(&mut e);
    set_time(&mut e.svm, deadline());
    sweep(&mut e).unwrap();
    e.svm.airdrop(&e.owner.pubkey(), SOL).unwrap();
    let owner = e.owner.insecure_clone();
    let ix = ix_update(owner.pubkey(), e.plan, INTERVAL, GRACE, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &owner, &[]), "PlanNotActive");
}

#[test]
fn req17_only_owner_can_update() {
    let mut e = new_env();
    init_ok(&mut e);
    let stranger = Keypair::new();
    e.svm.airdrop(&stranger.pubkey(), SOL).unwrap();
    let ix = ix_update(stranger.pubkey(), e.plan, INTERVAL, GRACE, sweep_bytes());
    assert!(send(&mut e.svm, &[ix], &stranger, &[]).is_err());
}

// ================= REQ14 keeper nonce burn (off-chain) =================


#[test]
fn extra_rejects_default_beneficiary() {
    let mut e = new_env();
    let ix = ix_init(&e, Pubkey::default(), e.keeper.pubkey(), e.nonce, INTERVAL, GRACE, sweep_bytes());
    assert_err(send(&mut e.svm, &[ix], &e.owner.insecure_clone(), &[]), "InvalidBeneficiary");
}

#[test]
fn req25_plan_account_size_matches_layout() {
    // 8 discriminator + 4*32 pubkeys + 3*8 i64 + status + bump + (4 + 1232) sweep_tx
    assert_eq!(8 + <kimida::Plan as anchor_lang::Space>::INIT_SPACE, 8 + 128 + 24 + 1 + 1 + 4 + MAX_SWEEP_TX_LEN);
}

// REQ16, REQ21, REQ22, REQ23, REQ24, REQ26 are off-chain: see keeper/tests/e2e_multi_user.rs.
