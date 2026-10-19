//! Owner-side helpers (REQ05, REQ10) plus read helpers shared with the keeper.

use anchor_lang::{system_program, AccountDeserialize, InstructionData, ToAccountMetas};
use anyhow::{anyhow, bail, Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},

    transaction::Transaction,
};
use solana_system_interface::instruction as sysix;

pub use kimida::Plan;

/// Size of a System Program nonce account.
pub const NONCE_ACCOUNT_LEN: usize = 80;

pub struct NonceInfo {
    pub authority: Pubkey,
    /// The stored durable nonce; used as the `recent_blockhash` of the sweep tx.
    pub blockhash: Hash,
}

/// REQ04: `Plan` PDA, seeds ["plan", owner].
pub fn plan_pda(owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[kimida::PLAN_SEED, owner.as_ref()], &kimida::id())
}

/// Parses a System Program nonce account:
/// version | state | authority | durable nonce | fee calculator.
///
/// Only `Versions::Current` (1) in the `Initialized` (1) state is accepted. Legacy (0) nonces are
/// invalid for durable transactions on Solana, so a sweep built on one could never execute.
pub fn parse_nonce(data: &[u8]) -> Result<NonceInfo> {
    if data.len() < NONCE_ACCOUNT_LEN {
        bail!("nonce account data is too short");
    }

    let version = u32::from_le_bytes(data[0..4].try_into()?);
    let state = u32::from_le_bytes(data[4..8].try_into()?);

    if version != 1 || state != 1 {
        bail!("not a supported initialized nonce account (version={version}, state={state})");
    }

    Ok(NonceInfo {
        authority: Pubkey::try_from(&data[8..40])?,
        blockhash: Hash::new_from_array(data[40..72].try_into()?),
    })
}

pub fn read_nonce(rpc: &RpcClient, nonce: &Pubkey) -> Result<NonceInfo> {
    let acc = rpc.get_account(nonce).context("fetch nonce account")?;
    if acc.owner != system_program::ID {
        bail!("nonce account not owned by system program");
    }
    parse_nonce(&acc.data)
}

pub fn fetch_plan(rpc: &RpcClient, owner: &Pubkey) -> Result<Plan> {
    let acc = rpc
        .get_account(&plan_pda(owner).0)
        .context("no plan for this owner")?;
    Ok(Plan::try_deserialize(&mut acc.data.as_slice())?)
}

/// REQ05: create a durable nonce account with `authority = keeper`; owner funds the rent.
pub fn create_nonce_account(rpc: &RpcClient, owner: &Keypair, keeper: &Pubkey) -> Result<Keypair> {
    let nonce = Keypair::new();
    let rent = rpc.get_minimum_balance_for_rent_exemption(NONCE_ACCOUNT_LEN)?;
    let ixs = sysix::create_nonce_account(&owner.pubkey(), &nonce.pubkey(), keeper, rent);
    send(rpc, &ixs, owner, &[&nonce])?;
    Ok(nonce)
}

fn heir_sweep_ix(owner: &Pubkey, beneficiary: &Pubkey, keeper: &Pubkey, nonce: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::HeirSweep {}.data(),
        kimida::accounts::HeirSweep {
            owner: *owner,
            keeper: *keeper,
            beneficiary: *beneficiary,
            plan: plan_pda(owner).0,
            nonce_account: *nonce,
            instructions: solana_sdk::sysvar::instructions::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

/// REQ10: `[AdvanceNonceAccount, heir_sweep]`, blockhash = durable nonce, fee payer = keeper,
/// signed by the owner only. The keeper signature slot stays empty. Returns bincode bytes.
pub fn build_partial_sweep(
    rpc: &RpcClient,
    owner: &Keypair,
    beneficiary: &Pubkey,
    keeper: &Pubkey,
    nonce: &Pubkey,
) -> Result<Vec<u8>> {
    let info = read_nonce(rpc, nonce)?;
    if info.authority != *keeper {
        bail!("nonce authority {} is not the keeper {}", info.authority, keeper);
    }
    let ixs = [
        sysix::advance_nonce_account(nonce, keeper),
        heir_sweep_ix(&owner.pubkey(), beneficiary, keeper, nonce),
    ];
    let msg = Message::new_with_blockhash(&ixs, Some(keeper), &info.blockhash);
    let mut tx = Transaction::new_unsigned(msg);
    tx.partial_sign(&[owner], info.blockhash);
    let bytes = bincode::serialize(&tx)?;
    if bytes.len() > kimida::MAX_SWEEP_TX_LEN {
        bail!("partial sweep is {} bytes (max {})", bytes.len(), kimida::MAX_SWEEP_TX_LEN);
    }
    Ok(bytes)
}

/// UC1. The nonce account must already exist (see `create_nonce_account`).
pub fn initialize_plan(
    rpc: &RpcClient,
    owner: &Keypair,
    beneficiary: &Pubkey,
    keeper: &Pubkey,
    nonce: &Pubkey,
    interval: i64,
    grace: i64,
) -> Result<()> {
    let sweep_tx = build_partial_sweep(rpc, owner, beneficiary, keeper, nonce)?;
    let ix = Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::InitializePlan {
            beneficiary: *beneficiary,
            keeper: *keeper,
            interval,
            grace,
            sweep_tx,
        }
        .data(),
        kimida::accounts::InitializePlan {
            owner: owner.pubkey(),
            plan: plan_pda(&owner.pubkey()).0,
            nonce_account: *nonce,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );
    send(rpc, &[ix], owner, &[])
}

/// Rebuilds the partial sweep from the stored plan.
fn fresh_sweep(rpc: &RpcClient, owner: &Keypair) -> Result<Vec<u8>> {
    let plan = fetch_plan(rpc, &owner.pubkey())?;
    build_partial_sweep(rpc, owner, &plan.beneficiary, &plan.keeper, &plan.nonce_account)
}

/// UC2: re-sign the partial sweep and reset the timer.
pub fn check_in(rpc: &RpcClient, owner: &Keypair) -> Result<()> {
    let sweep_tx = fresh_sweep(rpc, owner)?;
    let ix = Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::CheckIn { sweep_tx }.data(),
        kimida::accounts::CheckIn {
            owner: owner.pubkey(),
            plan: plan_pda(&owner.pubkey()).0,
        }
        .to_account_metas(None),
    );
    send(rpc, &[ix], owner, &[])
}

/// UC5.
pub fn update_plan(rpc: &RpcClient, owner: &Keypair, interval: i64, grace: i64) -> Result<()> {
    let sweep_tx = fresh_sweep(rpc, owner)?;
    let ix = Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::UpdatePlan { interval, grace, sweep_tx }.data(),
        kimida::accounts::UpdatePlan {
            owner: owner.pubkey(),
            plan: plan_pda(&owner.pubkey()).0,
        }
        .to_account_metas(None),
    );
    send(rpc, &[ix], owner, &[])
}

/// UC4. The keeper burns the nonce afterwards (REQ14).
pub fn close_plan(rpc: &RpcClient, owner: &Keypair) -> Result<()> {
    let ix = Instruction::new_with_bytes(
        kimida::id(),
        &kimida::instruction::ClosePlan {}.data(),
        kimida::accounts::ClosePlan {
            owner: owner.pubkey(),
            plan: plan_pda(&owner.pubkey()).0,
        }
        .to_account_metas(None),
    );
    send(rpc, &[ix], owner, &[])
}

/// Sign with `payer` (+ `extra`) and confirm.
pub fn send(rpc: &RpcClient, ixs: &[Instruction], payer: &Keypair, extra: &[&Keypair]) -> Result<()> {
    let blockhash = rpc.get_latest_blockhash()?;
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(ixs, Some(&payer.pubkey()), &signers, blockhash);
    rpc.send_and_confirm_transaction(&tx)
        .map(|_| ())
        .map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests below originate from Tushar Kumar's PR #1; the legacy-version case was changed to
    // assert rejection (legacy nonces cannot validate durable transactions).

    fn nonce_data(version: u32, state: u32, authority: &Pubkey, blockhash: &Hash) -> Vec<u8> {
        let mut data = vec![0u8; NONCE_ACCOUNT_LEN];
        data[0..4].copy_from_slice(&version.to_le_bytes());
        data[4..8].copy_from_slice(&state.to_le_bytes());
        data[8..40].copy_from_slice(authority.as_ref());
        data[40..72].copy_from_slice(blockhash.as_ref());
        data
    }

    #[test]
    fn parse_nonce_rejects_short_data() {
        let data = vec![0u8; NONCE_ACCOUNT_LEN - 1];
        assert!(parse_nonce(&data).is_err());
    }

    #[test]
    fn parse_nonce_rejects_uninitialized_state() {
        let data = nonce_data(1, 0, &Pubkey::new_unique(), &Hash::new_unique());
        assert!(parse_nonce(&data).is_err());
    }

    #[test]
    fn parse_nonce_rejects_unsupported_version() {
        let data = nonce_data(99, 1, &Pubkey::new_unique(), &Hash::new_unique());
        assert!(parse_nonce(&data).is_err());
    }

    #[test]
    fn parse_nonce_rejects_legacy_version() {
        let data = nonce_data(0, 1, &Pubkey::new_unique(), &Hash::new_unique());
        assert!(parse_nonce(&data).is_err());
    }

    #[test]
    fn parse_nonce_accepts_current_version() {
        let authority = Pubkey::new_unique();
        let blockhash = Hash::new_unique();
        let parsed = parse_nonce(&nonce_data(1, 1, &authority, &blockhash)).unwrap();

        assert_eq!(parsed.authority, authority);
        assert_eq!(parsed.blockhash, blockhash);
    }
}
