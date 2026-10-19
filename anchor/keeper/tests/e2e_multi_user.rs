//! End-to-end, multi-user flow on a throwaway `solana-test-validator`.
//!
//! Covers the off-chain requirements from the Deliverable 2 PDF:
//!   REQ16 nonce burn after close, REQ21 owner creates the nonce, REQ22 owner partially signs,
//!   REQ23 keeper scan, REQ24 simulate-then-broadcast, REQ26 finalized hard guard + safety margin.
//!
//! Scenario: owners A, B, C, D with separate beneficiaries (C's heir is A's own wallet), interleaved
//! create / check-in / sweep / cancel. Asserts exact balances and statuses after each step.
//!
//! The test builds the program itself with `--features short-timers` (second-scale timers) and
//! starts its own validator; it is skipped with a message if `solana-test-validator` or
//! `cargo build-sbf` are not installed. Takes about 2-3 minutes.

use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use kimida::PlanStatus;
use kimida_keeper::{config::Config, scan};
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{write_keypair_file, Keypair, Signer},
};

const SOL: u64 = 1_000_000_000;

struct Chain {
    child: Child,
    dir: PathBuf,
    rpc: RpcClient,
    url: String,
}

impl Drop for Chain {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn start_chain() -> Option<Chain> {
    if !have("solana-test-validator", "--version") {
        eprintln!("SKIPPED e2e_multi_user: solana-test-validator not found on PATH");
        return None;
    }

    let dir = std::env::temp_dir().join(format!("kimida-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // Program with second-scale timers (never use this build outside localnet).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../programs/kimida/Cargo.toml");
    let deploy = dir.join("deploy");
    let status = Command::new("cargo")
        .args(["build-sbf", "--features", "short-timers", "--manifest-path"])
        .arg(&manifest)
        .arg("--sbf-out-dir")
        .arg(&deploy)
        .status()
        .expect("cargo build-sbf failed to start");
    assert!(status.success(), "building the program with short-timers failed");

    let (rpc_port, faucet_port) = (free_port(), free_port());
    let child = Command::new("solana-test-validator")
        .args(["--reset", "--quiet", "--ledger"])
        .arg(dir.join("ledger"))
        .args(["--rpc-port", &rpc_port.to_string(), "--faucet-port", &faucet_port.to_string()])
        .arg("--bpf-program")
        .arg(kimida::id().to_string())
        .arg(deploy.join("kimida.so"))
        .stdout(std::fs::File::create(dir.join("validator.log")).unwrap())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start solana-test-validator");

    let url = format!("http://127.0.0.1:{rpc_port}");
    let rpc = RpcClient::new_with_commitment(url.clone(), CommitmentConfig::confirmed());
    let chain = Chain { child, dir, rpc, url };
    let t0 = Instant::now();
    while chain.rpc.get_version().is_err() {
        assert!(t0.elapsed() < Duration::from_secs(120), "validator did not start");
        std::thread::sleep(Duration::from_millis(500));
    }
    Some(chain)
}

fn airdrop(rpc: &RpcClient, to: &Pubkey, sol: u64) {
    let sig = rpc.request_airdrop(to, sol * SOL).unwrap();
    let t0 = Instant::now();
    while !rpc.confirm_transaction(&sig).unwrap_or(false) {
        assert!(t0.elapsed() < Duration::from_secs(30), "airdrop not confirmed");
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn user(rpc: &RpcClient, funded: bool) -> Keypair {
    let k = Keypair::new();
    if funded {
        airdrop(rpc, &k.pubkey(), 10);
    }
    k
}

fn bal(rpc: &RpcClient, k: &Pubkey) -> u64 {
    rpc.get_balance(k).unwrap()
}

fn status(rpc: &RpcClient, owner: &Pubkey) -> PlanStatus {
    kimida_client::fetch_plan(rpc, owner).unwrap().status
}

/// Run keeper scans until `owner`'s plan reaches `want` (or time out). Deadlines are judged on the
/// finalized Clock (REQ26), which lags wall time, so polling beats fixed sleeps.
fn scan_until(cfg: &Config, rpc: &RpcClient, owner: &Pubkey, want: PlanStatus) {
    let t0 = Instant::now();
    while status(rpc, owner) != want {
        assert!(t0.elapsed() < Duration::from_secs(120), "plan never reached {want:?}");
        if let Err(e) = scan::run_scan(cfg) {
            eprintln!("scan error (retrying): {e:#}");
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

fn finalized_clock(rpc: &RpcClient) -> i64 {
    let acc = rpc
        .get_account_with_commitment(&solana_sdk::sysvar::clock::ID, CommitmentConfig::finalized())
        .unwrap()
        .value
        .unwrap();
    bincode::deserialize::<solana_sdk::clock::Clock>(&acc.data).unwrap().unix_timestamp
}

#[test]
fn multi_user_flow() {
    let Some(chain) = start_chain() else { return };
    let rpc = &chain.rpc;

    // ---- actors ----
    let keeper = Keypair::new();
    let keeper_path = chain.dir.join("keeper.json");
    write_keypair_file(&keeper, &keeper_path).unwrap();
    airdrop(rpc, &keeper.pubkey(), 20);
    let mut cfg = Config {
        rpc: chain.url.clone(),
        keypair: keeper_path.to_string_lossy().into(),
        state: chain.dir.join("keeper-state.json").to_string_lossy().into(),
        safety_margin: 0,
        expected_genesis_hash: None,
    };

    let (a, b, c, d) = (user(rpc, true), user(rpc, true), user(rpc, true), user(rpc, true));
    let (ba, bb, bd) = (user(rpc, false), user(rpc, false), user(rpc, false));
    let kp = keeper.pubkey();

    // REQ21 + REQ22 + UC1 happen inside `setup`: owner creates the nonce (authority = keeper),
    // builds + partially signs [AdvanceNonce, heir_sweep], then initialize_plan.
    let setup = |owner: &Keypair, heir: &Pubkey, interval: i64, grace: i64| -> Pubkey {
        let nonce = kimida_client::create_nonce_account(rpc, owner, &kp).unwrap();
        kimida_client::initialize_plan(rpc, owner, heir, &kp, &nonce.pubkey(), interval, grace).unwrap();
        nonce.pubkey()
    };

    // ---- 1-2. A and B create ----
    let a_nonce = setup(&a, &ba.pubkey(), 12, 2);
    assert_eq!(kimida_client::read_nonce(rpc, &a_nonce).unwrap().authority, kp, "REQ21 nonce authority");
    assert_eq!(status(rpc, &a.pubkey()), PlanStatus::Active);
    setup(&b, &bb.pubkey(), 40, 2);
    assert_eq!(status(rpc, &b.pubkey()), PlanStatus::Active);

    // ---- 3. nothing due yet ----
    scan::run_scan(&cfg).unwrap();
    assert_eq!(bal(rpc, &ba.pubkey()), 0);

    // ---- 4. REQ26: past the deadline, a huge safety margin still blocks signing ----
    let a_deadline = kimida_client::fetch_plan(rpc, &a.pubkey()).unwrap().deadline();
    let t0 = Instant::now();
    while finalized_clock(rpc) < a_deadline {
        assert!(t0.elapsed() < Duration::from_secs(90));
        std::thread::sleep(Duration::from_secs(2));
    }
    cfg.safety_margin = 1_000_000;
    scan::run_scan(&cfg).unwrap();
    assert_eq!(status(rpc, &a.pubkey()), PlanStatus::Active, "REQ26 hard guard must hold the sweep");
    assert_eq!(bal(rpc, &ba.pubkey()), 0);

    // ---- 4b. margin lifted: A's beneficiary is swept, B is not ----
    cfg.safety_margin = 0;
    let a_bal = bal(rpc, &a.pubkey());
    scan_until(&cfg, rpc, &a.pubkey(), PlanStatus::Distributed);
    assert_eq!(bal(rpc, &ba.pubkey()), a_bal, "bA receives A's full live balance");
    assert_eq!(bal(rpc, &a.pubkey()), 0);
    assert_eq!(status(rpc, &b.pubkey()), PlanStatus::Active);

    // ---- 5. C creates (heir = A's wallet); D creates then cancels ----
    let a_before = bal(rpc, &a.pubkey());
    let c_bal = bal(rpc, &c.pubkey());
    setup(&c, &a.pubkey(), 12, 2);
    let d_nonce = setup(&d, &bd.pubkey(), 600, 600);
    scan::run_scan(&cfg).unwrap(); // keeper must see D's plan to learn its nonce (known gap)
    kimida_client::close_plan(rpc, &d).unwrap();

    // ---- 6. REQ16: closed plan -> nonce burned; A not swept twice ----
    scan::run_scan(&cfg).unwrap();
    assert!(rpc.get_account(&d_nonce).is_err(), "REQ16: D's nonce account must be gone");
    assert_eq!(bal(rpc, &bd.pubkey()), 0, "cancelled plan moves nothing");
    assert_eq!(bal(rpc, &ba.pubkey()), a_bal, "A is not swept twice");

    // ---- 7. B checks in (timer reset) ----
    kimida_client::check_in(rpc, &b).unwrap();
    assert_eq!(status(rpc, &b.pubkey()), PlanStatus::Active);

    // ---- 8. C's deadline passes -> funds land in A's wallet; B still waits ----
    let c_bal_now = bal(rpc, &c.pubkey());
    assert!(c_bal_now <= c_bal);
    scan_until(&cfg, rpc, &c.pubkey(), PlanStatus::Distributed);
    assert_eq!(bal(rpc, &a.pubkey()), a_before + c_bal_now, "A's wallet receives C's balance");
    assert_eq!(bal(rpc, &c.pubkey()), 0);
    assert_eq!(status(rpc, &b.pubkey()), PlanStatus::Active);

    // ---- 9. B's reset deadline passes -> bB swept ----
    let b_bal = bal(rpc, &b.pubkey());
    scan_until(&cfg, rpc, &b.pubkey(), PlanStatus::Distributed);
    assert_eq!(bal(rpc, &bb.pubkey()), b_bal, "bB receives B's full live balance");

    // ---- 10. final scan changes nothing ----
    let (x, y) = (bal(rpc, &ba.pubkey()), bal(rpc, &bb.pubkey()));
    scan::run_scan(&cfg).unwrap();
    assert_eq!((bal(rpc, &ba.pubkey()), bal(rpc, &bb.pubkey())), (x, y));
}
