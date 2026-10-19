//! REQ11: scan Plan PDAs, verify, co-sign and broadcast only after the deadline.

use std::time::{SystemTime, UNIX_EPOCH};

use anchor_lang::{AccountDeserialize, Discriminator};
use anyhow::{bail, Context, Result};
use kimida::{Plan, PlanStatus};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcSimulateTransactionConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_account_decoder_client_types::UiAccountEncoding;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::{
    burn::burn_nonce,
    config::Config,
    state::{State, Tracked},
};

/// REQ23: one pass over every Plan owned by this keeper.
pub fn run_scan(cfg: &Config) -> Result<()> {
    let rpc = cfg.rpc();
    if let Some(expected) = &cfg.expected_genesis_hash {
        let actual = rpc.get_genesis_hash()?.to_string();
        if &actual != expected {
            bail!("wrong cluster: RPC genesis {actual} != expected {expected}");
        }
    }
    let keeper = cfg.load_keypair()?;
    let mut state = State::load(&cfg.state)?;
    let now = chain_time(&rpc, CommitmentConfig::confirmed());

    let plans = fetch_plans(&rpc, &keeper.pubkey())?;
    println!("scan: {} plan(s) for keeper, chain time {now}", plans.len());

    for (pda, plan) in &plans {
        let key = pda.to_string();
        // Plan re-created with a new nonce: the old one must be burned (REQ14).
        if let Some(t) = state.plans.get(&key) {
            if t.nonce != plan.nonce_account.to_string() {
                burn_tracked(&rpc, &keeper, t);
            }
        }
        state.plans.insert(
            key,
            Tracked { owner: plan.owner.to_string(), nonce: plan.nonce_account.to_string() },
        );

        if !is_due(plan, now) {
            continue;
        }
        println!("plan {pda}: due, running preflight");
        let sent = preflight(&rpc, &keeper, plan, pda)
            .and_then(|tx| hard_guard(&rpc, pda, cfg.safety_margin).map(|_| tx))
            .and_then(|tx| sign_and_simulate(&rpc, &keeper, tx))
            .and_then(|tx| broadcast(&rpc, tx));
        match sent {
            Ok(sig) => println!("plan {pda}: sweep sent {sig}"),
            Err(e) => eprintln!("plan {pda}: skipped: {e:#}"),
        }
    }

    // Tracked plans that no longer exist were closed by the owner: burn their nonces.
    let live: Vec<String> = plans.iter().map(|(p, _)| p.to_string()).collect();
    let closed: Vec<String> = state.plans.keys().filter(|k| !live.contains(k)).cloned().collect();
    for key in closed {
        let t = state.plans.remove(&key).unwrap();
        println!("plan {key}: closed, burning nonce {}", t.nonce);
        burn_tracked(&rpc, &keeper, &t);
    }

    state.save(&cfg.state)?;
    Ok(())
}

fn burn_tracked(rpc: &RpcClient, keeper: &Keypair, t: &Tracked) {
    let res = (|| -> Result<()> {
        let nonce: Pubkey = t.nonce.parse()?;
        let owner: Pubkey = t.owner.parse()?;
        burn_nonce(rpc, keeper, &nonce, &owner)
    })();
    if let Err(e) = res {
        eprintln!("nonce burn failed for {}: {e:#}", t.nonce);
    }
}

/// The `Clock` sysvar's unix_timestamp: exactly what the on-chain deadline gate reads.
/// (Block time can run ahead of it, which would make the keeper think a plan is due too early.)
fn chain_time(rpc: &RpcClient, commitment: CommitmentConfig) -> i64 {
    rpc.get_account_with_commitment(&solana_sdk::sysvar::clock::ID, commitment)
        .ok()
        .and_then(|r| r.value)
        .and_then(|a| bincode::deserialize::<solana_sdk::clock::Clock>(&a.data).ok())
        .map(|c| c.unix_timestamp)
        .unwrap_or_else(|| SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64)
}

/// REQ26 hard guard, run immediately before signing: re-read the Plan at *finalized*
/// commitment and require finalized Clock >= last_checkin + interval + grace + safety margin.
/// Finalized state cannot roll back, so a fork or stale read can never make us sign early.
pub fn hard_guard(rpc: &RpcClient, plan_pda: &Pubkey, safety_margin: i64) -> Result<()> {
    let fin = CommitmentConfig::finalized();
    let acc = rpc
        .get_account_with_commitment(plan_pda, fin)?
        .value
        .context("plan not visible at finalized commitment yet")?;
    let plan = Plan::try_deserialize(&mut acc.data.as_slice())?;
    if plan.status != PlanStatus::Active {
        bail!("plan is not Active at finalized commitment");
    }
    let now = chain_time(rpc, fin);
    let need = plan.deadline() + safety_margin;
    if now < need {
        bail!("hard guard: finalized clock {now} < deadline + margin {need}");
    }
    Ok(())
}

/// All `Plan` accounts whose `keeper` field is this keeper.
pub fn fetch_plans(rpc: &RpcClient, keeper: &Pubkey) -> Result<Vec<(Pubkey, Plan)>> {
    let cfg = RpcProgramAccountsConfig {
        filters: Some(vec![
            RpcFilterType::Memcmp(Memcmp::new_base58_encoded(0, Plan::DISCRIMINATOR)),
            // 8 (discriminator) + owner (32) + beneficiary (32)
            RpcFilterType::Memcmp(Memcmp::new_base58_encoded(72, &keeper.to_bytes())),
        ]),
        // Plan accounts are >128 bytes, so base58 (the default) is rejected.
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            ..Default::default()
        },
        ..Default::default()
    };
    #[allow(deprecated)]
    let accounts = rpc.get_program_accounts_with_config(&kimida::id(), cfg)?;
    accounts
        .into_iter()
        .map(|(pk, acc)| Ok((pk, Plan::try_deserialize(&mut acc.data.as_slice())?)))
        .collect()
}

/// REQ06 mirror: Active and deadline reached.
pub fn is_due(plan: &Plan, now: i64) -> bool {
    plan.status == PlanStatus::Active && now >= plan.deadline()
}

/// Verifies the stored sweep ourselves, then returns it with the keeper signature added.
/// A premature or malformed submission would burn the nonce (POC fragility), so be strict.
pub fn preflight(rpc: &RpcClient, keeper: &Keypair, plan: &Plan, plan_pda: &Pubkey) -> Result<Transaction> {
    let tx: Transaction =
        bincode::deserialize(&plan.sweep_tx).context("sweep_tx is not a valid transaction")?;
    let msg = &tx.message;

    check_structure(msg, &keeper.pubkey())?;
    for needed in [&plan.owner, &plan.beneficiary, &plan.nonce_account, plan_pda] {
        if !msg.account_keys.contains(needed) {
            bail!("sweep tx does not reference {needed}");
        }
    }

    // Nonce still valid: stored value must equal the tx blockhash and authority must be us.
    let nonce = kimida_client::read_nonce(rpc, &plan.nonce_account)?;
    if nonce.authority != keeper.pubkey() {
        bail!("nonce authority is not the keeper");
    }
    if nonce.blockhash != msg.recent_blockhash {
        bail!("nonce advanced since this sweep was signed (stale copy)");
    }

    // Owner signature present and valid; the keeper slot is still empty.
    let results = tx.verify_with_results();
    let owner_idx = tx.message.account_keys.iter().position(|k| *k == plan.owner).unwrap();
    if !results[owner_idx] {
        bail!("owner signature missing or invalid");
    }

    Ok(tx)
}

const COMPUTE_BUDGET_ID: Pubkey =
    anchor_lang::prelude::pubkey!("ComputeBudget111111111111111111111111111111");
/// System Program instruction index of `AdvanceNonceAccount`.
const ADVANCE_NONCE_IX: u32 = 4;

/// Shape of the owner-signed sweep: fee payer = keeper, instruction 0 = System `AdvanceNonceAccount`
/// (required for a durable-nonce transaction), the last instruction = KimiDa `heir_sweep`, and
/// only Compute Budget instructions in between. Wallets commonly add those for priority fees, and
/// they cannot move funds, so they are allowed; anything else is rejected.
pub fn check_structure(msg: &solana_sdk::message::Message, keeper: &Pubkey) -> Result<()> {
    if msg.account_keys.first() != Some(keeper) {
        bail!("fee payer is not the keeper");
    }
    let n = msg.instructions.len();
    if n < 2 {
        bail!("expected at least [AdvanceNonce, heir_sweep], got {n} instruction(s)");
    }
    let program_of = |i: usize| msg.account_keys[msg.instructions[i].program_id_index as usize];

    let first = &msg.instructions[0];
    let is_advance = program_of(0) == anchor_lang::system_program::ID
        && first.data.len() >= 4
        && u32::from_le_bytes(first.data[0..4].try_into().unwrap()) == ADVANCE_NONCE_IX;
    if !is_advance {
        bail!(
            "instruction 0 is not AdvanceNonceAccount (is it {}? a wallet may have reordered the transaction)",
            program_of(0)
        );
    }
    if program_of(n - 1) != kimida::id() {
        bail!("last instruction is not heir_sweep");
    }
    for i in 1..n - 1 {
        if program_of(i) != COMPUTE_BUDGET_ID {
            bail!("unexpected program {} at instruction {i}", program_of(i));
        }
    }
    Ok(())
}

/// REQ24: add the keeper signature, then dry-run. Failing here avoids burning the nonce on-chain.
fn sign_and_simulate(rpc: &RpcClient, keeper: &Keypair, mut tx: Transaction) -> Result<Transaction> {
    tx.partial_sign(&[keeper], tx.message.recent_blockhash);
    let sim = rpc.simulate_transaction_with_config(
        &tx,
        RpcSimulateTransactionConfig {
            sig_verify: true,
            // Same commitment as the Clock read in `chain_time`; the default (finalized) lags ~12s.
            commitment: Some(CommitmentConfig::confirmed()),
            ..Default::default()
        },
    )?;
    if let Some(err) = sim.value.err {
        bail!("simulation failed: {err:?} logs={:?}", sim.value.logs);
    }
    Ok(tx)
}

fn broadcast(rpc: &RpcClient, tx: Transaction) -> Result<solana_sdk::signature::Signature> {
    Ok(rpc.send_and_confirm_transaction(&tx)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(last: i64, status: PlanStatus) -> Plan {
        Plan {
            owner: Pubkey::new_unique(),
            beneficiary: Pubkey::new_unique(),
            keeper: Pubkey::new_unique(),
            nonce_account: Pubkey::new_unique(),
            checkin_interval: 100,
            grace_period: 20,
            last_checkin: last,
            status,
            bump: 255,
            sweep_tx: vec![],
        }
    }

    #[test]
    fn due_only_at_or_after_deadline() {
        let p = plan(1000, PlanStatus::Active);
        assert!(!is_due(&p, 1119));
        assert!(is_due(&p, 1120));
    }

    mod structure {
        use super::super::*;
        use solana_sdk::{instruction::Instruction, message::Message};
        use solana_system_interface::instruction as sysix;

        fn sweep_ix() -> Instruction {
            Instruction::new_with_bytes(kimida::id(), &[0u8; 8], vec![])
        }
        fn budget_ix() -> Instruction {
            Instruction::new_with_bytes(COMPUTE_BUDGET_ID, &[2, 0, 0, 0, 0], vec![])
        }
        fn msg(keeper: &Pubkey, ixs: &[Instruction]) -> Message {
            Message::new(ixs, Some(keeper))
        }

        #[test]
        fn accepts_advance_then_sweep() {
            let (k, n) = (Pubkey::new_unique(), Pubkey::new_unique());
            let m = msg(&k, &[sysix::advance_nonce_account(&n, &k), sweep_ix()]);
            assert!(check_structure(&m, &k).is_ok());
        }

        #[test]
        fn accepts_compute_budget_between() {
            let (k, n) = (Pubkey::new_unique(), Pubkey::new_unique());
            let m = msg(&k, &[sysix::advance_nonce_account(&n, &k), budget_ix(), budget_ix(), sweep_ix()]);
            assert!(check_structure(&m, &k).is_ok());
        }

        #[test]
        fn rejects_compute_budget_before_advance() {
            // What a wallet that prepends priority-fee instructions produces: not a nonce tx.
            let (k, n) = (Pubkey::new_unique(), Pubkey::new_unique());
            let m = msg(&k, &[budget_ix(), budget_ix(), sysix::advance_nonce_account(&n, &k), sweep_ix()]);
            assert!(check_structure(&m, &k).is_err());
        }

        #[test]
        fn rejects_foreign_instruction_between() {
            let (k, n, o) = (Pubkey::new_unique(), Pubkey::new_unique(), Pubkey::new_unique());
            let m = msg(&k, &[sysix::advance_nonce_account(&n, &k), sysix::transfer(&k, &o, 1), sweep_ix()]);
            assert!(check_structure(&m, &k).is_err());
        }

        #[test]
        fn rejects_wrong_fee_payer() {
            let (k, n, other) = (Pubkey::new_unique(), Pubkey::new_unique(), Pubkey::new_unique());
            let m = msg(&other, &[sysix::advance_nonce_account(&n, &k), sweep_ix()]);
            assert!(check_structure(&m, &k).is_err());
        }

        #[test]
        fn rejects_missing_sweep() {
            let (k, n) = (Pubkey::new_unique(), Pubkey::new_unique());
            let m = msg(&k, &[sysix::advance_nonce_account(&n, &k), budget_ix()]);
            assert!(check_structure(&m, &k).is_err());
        }
    }

    #[test]
    fn distributed_is_never_due() {
        assert!(!is_due(&plan(0, PlanStatus::Distributed), i64::MAX));
    }
}
