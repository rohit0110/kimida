# KimiDa: Anchor workspace

Non-custodial digital succession for Solana wallets. The owner keeps all assets in their own wallet and pre-signs a sweep to a beneficiary; if the owner stops checking in, the KimiDa keeper co-signs and broadcasts it after the deadline.

Requirements (REQ01-REQ26) and use cases (UC1-UC5) are defined in `../Heirloom-Deliverable-2-Architecture-and-Requirements.pdf` and referenced throughout the code and tests.

**Status:** the on-chain program, owner client/CLI and keeper are implemented and verified end to end on a local validator (multi-user create / check-in / sweep / cancel). **Not deployed to devnet or mainnet, not audited.**

---

## 1. Components

```
anchor/
├── Anchor.toml
├── .env.example              # config template for keeper + client (copy to .env)
├── programs/kimida/        # ON-CHAIN: Anchor program (5 instructions)
│   ├── src/{lib,state,error,constants}.rs, instructions/*.rs
│   └── tests/plan_lifecycle.rs     # LiteSVM tests, one+ per on-chain REQ
├── client/                   # OWNER SIDE: lib + `kimida-client` CLI
│                             #   nonce creation (REQ21), partial-sweep signing (REQ22), UC1/2/4/5
├── keeper/                   # BACKEND: lib + `kimida-keeper` binary
│   ├── src/{scan,burn,state,config}.rs
│   └── tests/e2e_multi_user.rs     # real-validator, multi-user end-to-end
└── scripts/localnet.sh       # build with short timers + start a local validator
```

| Component | Runs where | Holds a key? | Job |
|-----------|-----------|--------------|-----|
| Program | Solana | no | Plan PDA state machine; enforces deadline, bindings, replay guard, nonce-at-ix-0 |
| Owner client | owner's machine / wallet app | owner key | Creates the nonce account (authority = keeper), signs the partial sweep, creates / checks in / updates / closes the plan |
| Keeper | a server you operate | **keeper key** | Scans plans, verifies, co-signs and broadcasts after the deadline, burns nonces after cancel |

| UC | Instruction | Handler file | REQs |
|----|-------------|--------------|------|
| UC1 | `initialize_plan` | `instructions/initialize_plan.rs` | 01-04, 25 |
| UC2 | `check_in` | `instructions/check_in.rs` | 05-07 |
| UC3 | `heir_sweep` | `instructions/heir_sweep.rs` | 08-14 |
| UC4 | `close_plan` | `instructions/close_plan.rs` | 15 (+16 in keeper) |
| UC5 | `update_plan` | `instructions/update_plan.rs` | 17-20 |

Off-chain: REQ16 (nonce burn), REQ21 (nonce creation), REQ22 (partial sweep), REQ23 (scan), REQ24 (simulate then broadcast), REQ26 (finalized hard guard).

## 2. Prerequisites

- Rust (pinned in `rust-toolchain.toml`)
- Solana CLI (Agave) 3.x: `solana`, `solana-test-validator`, `cargo build-sbf`
- Anchor CLI 1.1.x

## 3. Build and test

```bash
cd anchor
anchor build                 # program -> target/deploy/kimida.so + IDL (production bounds)
cargo test --workspace       # everything below, ~2 min (the e2e test dominates)
```

| Test | What | Covers |
|------|------|--------|
| `programs/kimida/tests/plan_lifecycle.rs` | LiteSVM. Test names carry the REQ id. Needs `anchor build` first (loads `target/deploy/kimida.so`) | REQ01-REQ15, REQ17-REQ20, REQ25 |
| `keeper/tests/e2e_multi_user.rs` | Starts its own validator, builds the program with `short-timers`, runs real keeper + client. Owners A, B, C, D with separate beneficiaries; interleaved create / check-in / sweep / cancel; exact balance and status assertions | REQ16, REQ21-REQ24, REQ26 |
| `keeper/src/scan.rs` unit tests | deadline logic | REQ23 |

The e2e test skips with a message if `solana-test-validator` is not installed. Run it alone: `cargo test -p kimida-keeper --test e2e_multi_user -- --nocapture`.

## 4. Configuration: chains, program ID, keys

**Solana has no chain id.** A cluster is identified by its **genesis hash** (`solana genesis-hash -u <cluster>`), and the program is identified by its **program ID**, which is compiled in (`declare_id!`). So "which chain" is two things you must keep consistent per environment:

| Setting | Where it lives | Notes |
|---------|----------------|-------|
| Cluster endpoint | `RPC_URL` (env / `.env` / `--rpc`) | default `http://127.0.0.1:8899` |
| Cluster identity | `KEEPER_EXPECTED_GENESIS_HASH` | the keeper refuses to run if the RPC's genesis hash differs. **Set this for every non-local deployment.** |
| Program ID | `declare_id!` in `programs/kimida/src/lib.rs` and `Anchor.toml` `[programs.<cluster>]` | one deployed program (and ideally one keypair) per cluster; `anchor keys sync` after changing `target/deploy/kimida-keypair.json` |
| Keeper key | `KEEPER_KEYPAIR` | the nonce authority, fee payer and co-signer for every plan naming it |

Config loading order (first wins): CLI flag, real environment variable, `./.env` (or a parent directory's), built-in default. `kimida-keeper` and `kimida-client` both load `.env`.

```bash
cp .env.example .env     # .env is gitignored; never commit it
```

| Variable | Used by | Meaning |
|----------|---------|---------|
| `RPC_URL` | keeper, client | Solana JSON-RPC endpoint |
| `KEEPER_EXPECTED_GENESIS_HASH` | keeper | cluster identity guard (recommended everywhere except localnet) |
| `KEEPER_KEYPAIR` | keeper | path to the keeper keypair file |
| `KEEPER_STATE` | keeper | path to the keeper's tracked-plan state file |
| `KEEPER_SAFETY_MARGIN` | keeper | seconds past the deadline (finalized Clock) before signing, REQ26. Default 300 |
| `OWNER_KEYPAIR` | client | owner keypair path (default `~/.config/solana/id.json`) |

Cluster genesis hashes: mainnet-beta `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`, devnet `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`. Localnet's changes on every `--reset`, so leave the guard off there.

Not configurable at runtime (compiled in): the program ID, the interval/grace bounds, and the 1,232-byte `sweep_tx` cap. A different bound or program ID means a different build.

## 5. Run on localnet (end to end)

The `short-timers` cargo feature lowers the minimum interval/grace to seconds so a demo is possible. **Never deploy a `short-timers` build to mainnet** (it would let a plan be created with a 5-second check-in). A clearly labelled devnet demo build is acceptable (see section 7).

```bash
# terminal 1: builds with short timers and starts a validator with the program preloaded
./scripts/localnet.sh

# terminal 2 (from anchor/)
cargo run -p kimida-keeper -- keygen                    # once; writes keeper/keeper-keypair.json (gitignored)
KEEPER=$(solana address -k keeper/keeper-keypair.json)
solana airdrop 5 $KEEPER -u localhost

solana-keygen new --no-bip39-passphrase -o /tmp/owner.json
solana-keygen new --no-bip39-passphrase -o /tmp/ben.json
solana airdrop 10 $(solana address -k /tmp/owner.json) -u localhost
export OWNER_KEYPAIR=/tmp/owner.json

# owner: create nonce (authority = keeper) + plan, interval 8s, grace 2s
cargo run -p kimida-client -- setup --keeper $KEEPER --beneficiary $(solana address -k /tmp/ben.json) --interval 8 --grace 2
cargo run -p kimida-client -- status

# keeper: too early, nothing happens
KEEPER_SAFETY_MARGIN=0 cargo run -p kimida-keeper -- scan
sleep 20
# keeper: past the deadline on the FINALIZED clock (which lags ~12s on localnet), co-signs and sweeps
KEEPER_SAFETY_MARGIN=0 cargo run -p kimida-keeper -- scan
# if it reports "hard guard: finalized clock ... < deadline", the finalized clock has not caught up yet: re-run the scan
solana balance $(solana address -k /tmp/ben.json) -u localhost
```

Other owner commands: `check-in`, `update --interval N --grace N`, `close` (the next keeper scan burns the nonce and returns its rent to the owner), `status`.

## 6. Running the keeper as a backend

The keeper is a long-running process (or a scheduled job). It is the only component that needs to be online for the protocol to work after setup.

```bash
cargo run --release -p kimida-keeper -- keygen     # once per environment
cargo run --release -p kimida-keeper -- info       # pubkey, balance, RPC genesis hash, program ID
cargo run --release -p kimida-keeper -- scan       # one pass (use from cron / a scheduler)
cargo run --release -p kimida-keeper -- run --every 3600   # or loop forever
```

Each pass (REQ23):
1. Fetch every `Plan` account whose `keeper` field is this keeper.
2. For plans past their deadline, run **preflight**: fee payer is the keeper; `AdvanceNonceAccount` first and `heir_sweep` last, with only Compute Budget instructions between; the tx references the plan's owner, beneficiary, nonce and PDA; nonce authority is the keeper; the nonce value still matches the signed tx (else the copy is stale); the owner signature is valid.
3. **Hard guard (REQ26):** re-read the plan and Clock at *finalized* commitment and require `Clock >= last_checkin + interval + grace + KEEPER_SAFETY_MARGIN`.
4. Add the keeper signature, **simulate** (REQ24), and only then broadcast.
5. Tracked plans that no longer exist (owner cancelled) get their nonce advanced then withdrawn, rent to the owner (REQ16).

### In Docker (local)

```bash
cd anchor
cp .env.example .env                 # RPC_URL, KEEPER_EXPECTED_GENESIS_HASH, KEEPER_SAFETY_MARGIN
./scripts/docker_load_key.sh keeper/devnet-keeper-keypair.json   # one-time: key -> `keeper-secrets` volume (via stdin)
docker compose up -d --build         # builds the keeper image, runs `kimida-keeper run --every 5`
docker compose logs -f keeper
docker compose down                  # stop; keeper state persists in the `keeper-state` volume
```

The key is never in the image or build context (`.dockerignore` excludes keypairs, `.env` and state files); the container runs as a non-root user with the key volume mounted read-only. Change the scan interval with `KEEPER_SCAN_EVERY`.

Operating it:
- **Key custody:** the keeper key is a plain file today. It has protocol-wide blast radius (it signs for every plan naming it). HSM/KMS and an N-of-M signer set are post-MVP; until then protect the file, fund it modestly, and rely on REQ26 plus on-chain checks. A compromised keeper can withhold or burn nonces (DoS) but cannot redirect funds: the beneficiary is inside the owner's signed message and re-checked on-chain.
- **Funding:** the keeper pays all sweep and nonce-burn fees. Keep a balance and alert on low funds.
- **State file:** `KEEPER_STATE` (`keeper-state.json`) maps plan -> (owner, nonce). Back it up and keep it on persistent storage; losing it means closed plans' nonces are no longer burned.
- **Interval:** choose `--every` well below the smallest `grace_period` you allow, so a due plan is not left waiting long after its deadline plus margin.
- **Production bounds:** the interval/grace min/max in `programs/kimida/src/constants.rs` are placeholders pending a decision.

**Open design question: plans per person.** The PDA allows one plan per wallet; nothing limits one person using many wallets. See `../frontend/NOTES.md`.

**Open design question: orphan nonces.** Not solved; needs a decision before this is a protocol.

An *orphan nonce* is a nonce account on-chain that belongs to no plan the keeper knows about. The keeper only learns a nonce exists by seeing a `Plan` that points at it. Setup is two steps (owner creates the nonce, then `initialize_plan`), so if an owner creates a plan and closes it between two keeper scans, or creates the nonce and never finishes `initialize_plan`, the keeper never sees it.

Effect: no funds are at risk (a stale signed sweep still needs the keeper signature and the plan is gone), but the nonce is never burned (REQ16), the owner's rent stays locked in the account, and the keeper no longer knows which owner to refund. The e2e test works around this by scanning between D's create and cancel.

Options to consider: (1) the client registers each nonce with the keeper at creation; (2) the keeper scans for nonce accounts with authority = keeper and no plan, burning them after a waiting period (owner unknown, so the rent refund needs a rule); (3) record the nonce's owner on-chain before the plan exists (beyond the PDF's five instructions). The keeper-creates-the-nonce variant would largely remove the problem.

## 7. Deploying to devnet / mainnet

**Devnet (demo):** `./scripts/deploy_devnet.sh` builds, checks the deployer's balance, deploys, creates and funds a devnet keeper key, and prints the `.env` values to use. It refuses to run against any cluster whose genesis hash is not devnet's. Set `DEMO_SHORT_TIMERS=1` for a seconds-scale build so a demo can complete live; without it the minimum check-in interval is one day. A deploy needs roughly 2.3 SOL on the deployer (3.3 recommended, for upgrade headroom), plus keeper funding.

**Mainnet:** not done. Checklist:

Checklist before any non-local deployment:
1. Build **without** `short-timers`: `anchor build`. Confirm the bounds in `constants.rs`.
2. Generate a fresh program keypair per cluster, set `declare_id!` and `Anchor.toml`, `anchor keys sync`, rebuild.
3. Deploy (`anchor deploy --provider.cluster devnet`); decide who holds the upgrade authority (multisig for mainnet, or revoke it).
4. Generate a fresh keeper key for that cluster, fund it, set `RPC_URL`, `KEEPER_EXPECTED_GENESIS_HASH`, `KEEPER_KEYPAIR`, `KEEPER_STATE` in that environment's `.env`.
5. Run owner setup against that cluster, confirm `keeper info` shows the right genesis hash and program ID.
6. Security review of the program and of keeper key handling before real funds.

## 8. Notes

- Frontend: `../frontend` (single page, wallet connect + create plan + sign). See its README.
- This is a subfolder of the `kimida` repo and has no git repo of its own (`anchor init --no-git`).
- Secrets are gitignored: `.env`, `keeper/keeper-keypair.json`, `keeper/keeper-state.json`, `*-keypair.json`.
