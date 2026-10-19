//! REQ14: after a plan is closed, advance then withdraw the nonce account (rent -> owner).
//! Every previously signed sweep is permanently invalid once the nonce moves.

use std::{thread::sleep, time::Duration};

use anyhow::{bail, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_system_interface::instruction as sysix;

pub fn burn_nonce(rpc: &RpcClient, keeper: &Keypair, nonce: &Pubkey, owner: &Pubkey) -> Result<()> {
    let Some(acc) = rpc.get_account_with_commitment(nonce, rpc.commitment())?.value else {
        return Ok(()); // already gone
    };
    let info = kimida_client::parse_nonce(&acc.data)?;
    if info.authority != keeper.pubkey() {
        bail!("keeper is not the authority of {nonce}");
    }

    // 1. Advance (invalidates every signed copy built on the old nonce value).
    kimida_client::send(rpc, &[sysix::advance_nonce_account(nonce, &keeper.pubkey())], keeper, &[])?;

    // 2. Withdrawing the full balance needs the stored nonce != current blockhash,
    //    so wait for a fresh blockhash first.
    let stored = kimida_client::read_nonce(rpc, nonce)?.blockhash;
    for _ in 0..60 {
        if rpc.get_latest_blockhash()? != stored {
            break;
        }
        sleep(Duration::from_millis(500));
    }
    let lamports = rpc.get_balance(nonce)?;
    kimida_client::send(
        rpc,
        &[sysix::withdraw_nonce_account(nonce, &keeper.pubkey(), owner, lamports)],
        keeper,
        &[],
    )?;
    println!("burned nonce {nonce}, rent {lamports} -> owner {owner}");
    Ok(())
}
