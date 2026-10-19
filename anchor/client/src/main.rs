//! Owner CLI for localnet demos: `kimida-client --help`.

use anyhow::Result;
use clap::{Parser, Subcommand};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{

    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
};

#[derive(Parser)]
struct Cli {
    #[arg(long, env = "RPC_URL", default_value = "http://127.0.0.1:8899", global = true)]
    rpc: String,
    /// Owner keypair file (defaults to the Solana CLI wallet).
    #[arg(long, env = "OWNER_KEYPAIR", default_value = "~/.config/solana/id.json", global = true)]
    owner: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create the nonce account (authority = keeper) and the plan (UC1).
    Setup {
        #[arg(long)]
        keeper: Pubkey,
        #[arg(long)]
        beneficiary: Pubkey,
        /// Seconds between required check-ins.
        #[arg(long)]
        interval: i64,
        /// Seconds of grace after the interval.
        #[arg(long)]
        grace: i64,
    },
    /// Reset the timer and re-sign the sweep (UC2).
    CheckIn,
    /// Change interval/grace, reset the timer, re-sign the sweep (UC5).
    Update {
        #[arg(long)]
        interval: i64,
        #[arg(long)]
        grace: i64,
    },
    /// Cancel the plan (UC4).
    Close,
    /// Print the plan.
    Status,
}

fn expand(p: &str) -> String {
    match p.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", std::env::var("HOME").unwrap_or_default()),
        None => p.to_string(),
    }
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let rpc = RpcClient::new_with_commitment(cli.rpc.clone(), CommitmentConfig::confirmed());
    let owner = read_keypair_file(expand(&cli.owner)).map_err(|e| anyhow::anyhow!("{e}"))?;

    match cli.cmd {
        Cmd::Setup { keeper, beneficiary, interval, grace } => {
            let nonce = kimida_client::create_nonce_account(&rpc, &owner, &keeper)?;
            println!("nonce account: {}", nonce.pubkey());
            kimida_client::initialize_plan(&rpc, &owner, &beneficiary, &keeper, &nonce.pubkey(), interval, grace)?;
            println!("plan created: {}", kimida_client::plan_pda(&owner.pubkey()).0);
        }
        Cmd::CheckIn => {
            kimida_client::check_in(&rpc, &owner)?;
            println!("checked in");
        }
        Cmd::Update { interval, grace } => {
            kimida_client::update_plan(&rpc, &owner, interval, grace)?;
            println!("plan updated");
        }
        Cmd::Close => {
            kimida_client::close_plan(&rpc, &owner)?;
            println!("plan closed");
        }
        Cmd::Status => {
            let p = kimida_client::fetch_plan(&rpc, &owner.pubkey())?;
            println!("owner:       {}", p.owner);
            println!("beneficiary: {}", p.beneficiary);
            println!("keeper:      {}", p.keeper);
            println!("nonce:       {}", p.nonce_account);
            println!("interval:    {}s  grace: {}s", p.checkin_interval, p.grace_period);
            println!("last_checkin {}  deadline {}", p.last_checkin, p.deadline());
            println!("status:      {:?}  sweep_tx: {} bytes", p.status, p.sweep_tx.len());
        }
    }
    Ok(())
}
