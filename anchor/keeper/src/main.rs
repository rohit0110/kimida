//! KimiDa keeper (REQ11, REQ14): nonce authority + fee payer + final co-signer.
//!
//!   kimida-keeper keygen            # create the keeper keypair (once)
//!   kimida-keeper info              # pubkey + balance
//!   kimida-keeper scan              # one pass
//!   kimida-keeper run --every 30    # loop

use anyhow::Result;
use clap::{Parser, Subcommand};
use solana_sdk::signature::{write_keypair_file, Keypair, Signer};

use kimida_keeper::{config::Config, scan};

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    cfg: Config,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a new keeper keypair at --keypair (refuses to overwrite).
    Keygen,
    /// Print the keeper pubkey and balance.
    Info,
    /// Run one scan pass.
    Scan,
    /// Run scan passes forever.
    Run {
        /// Seconds between passes.
        #[arg(long, default_value_t = 30)]
        every: u64,
    },
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok(); // load ./.env (or a parent's) before clap reads env vars
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Keygen => {
            let path = cli.cfg.keypair_path();
            if std::path::Path::new(&path).exists() {
                anyhow::bail!("{path} already exists; refusing to overwrite");
            }
            let kp = Keypair::new();
            write_keypair_file(&kp, &path).map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("keeper pubkey: {}\nwritten to {path}", kp.pubkey());
        }
        Cmd::Info => {
            let keeper = cli.cfg.load_keypair()?;
            let bal = cli.cfg.rpc().get_balance(&keeper.pubkey())?;
            let rpc = cli.cfg.rpc();
            println!("keeper {}  balance {} lamports", keeper.pubkey(), bal);
            println!("rpc {}  genesis {}", cli.cfg.rpc, rpc.get_genesis_hash()?);
            println!("program {}", kimida::id());
        }
        Cmd::Scan => scan::run_scan(&cli.cfg)?,
        Cmd::Run { every } => loop {
            if let Err(e) = scan::run_scan(&cli.cfg) {
                eprintln!("scan failed: {e:#}");
            }
            std::thread::sleep(std::time::Duration::from_secs(every));
        },
    }
    Ok(())
}
