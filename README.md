# KimiDa

Non-custodial digital succession for Solana wallets. You name a beneficiary and a check-in period.
While you keep checking in, **your assets never leave your own wallet**: no vault, no escrow, no shared
key. If you stop, a pre-authorised sweep moves your SOL to the beneficiary.

> **Status: devnet demo.** The program is deployed to devnet with *demo timers* (see
> [Devnet deployment](#2-devnet-deployment)). It is not audited and not deployed to mainnet.

---

## 1. How it works

```
 Owner (browser wallet)                    Solana                        Keeper (Docker backend)
 ----------------------                    ------                        -----------------------
 1. create durable nonce account  ───────► nonce account (authority = keeper)
 2. sign [AdvanceNonce, heir_sweep]
    (keeper signature left empty)
 3. initialize_plan(beneficiary,
    interval, grace, signed sweep) ───────► Plan PDA  ["plan", owner]
 4. "Sign" = check_in ─────────────────────► last_checkin = now (timer resets)
                                                                          5. scan Plan PDAs
                                                                          6. after deadline: verify,
                                                                             simulate, add its signature
                                           heir_sweep  ◄──────────────────  7. broadcast
                                           owner lamports ─► beneficiary
                                           plan.status = Distributed
 8. "Cancel plan" = close_plan ────────────► Plan closed; keeper burns the nonce
```

- The owner's signed sweep has **no amount**: it moves the owner's *live* balance at execution time.
- The sweep is **structurally incomplete** until the keeper adds its signature (the keeper is nonce
  authority and fee payer), so nobody holding the stored bytes can submit it early or burn the nonce.
- The keeper **cannot redirect funds**: the beneficiary is inside the owner's signed message and is
  re-checked on-chain against the plan.
- The keeper only signs when the **finalized** clock is past `last_checkin + interval + grace` plus a
  safety margin (REQ26).

Requirements (REQ01 to REQ26) and use cases (UC1 to UC5) are in
`Heirloom-Deliverable-2-Architecture-and-Requirements.pdf` and are referenced by name in tests and code.

## 2. Devnet deployment

| | |
|---|---|
| **Cluster** | Solana devnet (`https://api.devnet.solana.com`) |
| **Genesis hash** | `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` |
| **Program ID** | [`Egn9Kgqa8sLT2F5u5DS26Bg791eW51HfKKuqSSp8euke`](https://explorer.solana.com/address/Egn9Kgqa8sLT2F5u5DS26Bg791eW51HfKKuqSSp8euke?cluster=devnet) |
| **Loader** | `BPFLoaderUpgradeab1e11111111111111111111111` (upgradeable) |
| **ProgramData** | `6Toh7w4d1p7mVJvsWKSuFgbmovWKGeQrGW7GU2CCcAGh` |
| **Upgrade authority** | `3SbFWgoCKCKM9MP3VMeou9ABf4dg8fTS5daEHQNvF8SW` (deployer wallet) |
| **Last deployed slot** | 510218157 |
| **Binary size** | 197,744 bytes (ProgramData account is sized to this binary; a larger upgrade needs `solana program extend` first) |
| **Keeper** | `AsEvw9nKTdCjW3hK3RiJtwNb2rNwaf8EYZmyRABCFGBM` (nonce authority, fee payer and co-signer for every plan made from the UI) |
| **Build** | `anchor build -- --features short-timers` (**demo build**) |
| **Interval / grace bounds** | min check-in **5 s**, min grace **1 s**, max check-in 5 years, max grace 1 year |

**This is a demo build.** The `short-timers` feature lowers the minimum check-in interval from one day
to five seconds so a sweep can be shown live. A production build (`anchor build`, no feature) enforces a
1-day / 1-hour minimum. **Never deploy a `short-timers` build to mainnet.**

The program key (`target/deploy/kimida-keypair.json`) is the same dev key used on localnet; use a fresh
program keypair per cluster for anything real.

### Accounts and instructions

Plan PDA: seeds `["plan", owner]`, so **one plan per wallet**.

| Offset | Field | Type |
|-------:|-------|------|
| 0 | discriminator | 8 bytes |
| 8 | `owner` | Pubkey |
| 40 | `beneficiary` | Pubkey |
| 72 | `keeper` | Pubkey |
| 104 | `nonce_account` | Pubkey |
| 136 | `checkin_interval` | i64 (seconds) |
| 144 | `grace_period` | i64 (seconds) |
| 152 | `last_checkin` | i64 (unix) |
| 160 | `status` | u8 (0 Active, 1 Distributed) |
| 161 | `bump` | u8 |
| 162 | `sweep_tx` | Vec<u8> (u32 length + up to 1,232 bytes) |

| Instruction | Signers | What it does | UC / REQ |
|-------------|---------|--------------|----------|
| `initialize_plan(beneficiary, keeper, interval, grace, sweep_tx)` | owner | create the plan; checks bounds, nonce authority == keeper, nonce is initialized and current-version | UC1 / REQ01-04 |
| `check_in(sweep_tx)` | owner | reset `last_checkin`, overwrite `sweep_tx` | UC2 / REQ05-07 |
| `heir_sweep()` | owner (presigned) + keeper | after the deadline, transfer the owner's live balance to the beneficiary | UC3 / REQ08-14 |
| `update_plan(interval, grace, sweep_tx)` | owner | change the schedule, reset the timer | UC5 / REQ17-20 |
| `close_plan()` | owner | close the plan, return rent, delete `sweep_tx` | UC4 / REQ15 |

`heir_sweep` also requires instruction 0 of the transaction to be System `AdvanceNonceAccount` on
`plan.nonce_account` (REQ11). Custom errors are listed in `anchor/programs/kimida/src/error.rs`.

## 3. Repository layout

```
kimida/
├── README.md                 # this file
├── anchor/                   # on-chain program + owner CLI + keeper backend (Rust)
│   ├── programs/kimida/    #   Anchor program, LiteSVM tests
│   ├── client/               #   owner library + `kimida-client` CLI
│   ├── keeper/               #   `kimida-keeper` backend (+ multi-user e2e test)
│   ├── Dockerfile, docker-compose.yml
│   └── scripts/              #   localnet.sh, deploy_devnet.sh, docker_load_key.sh
├── frontend/                 # single-page UI (Vite + React + wallet-adapter)
└── deliverable-*, *.pdf      # course deliverables
```

More detail: `anchor/README.md` (program, keeper, tests, deployment) and `frontend/README.md`.

## 4. Run the demo (devnet)

### Prerequisites
- Docker Desktop (running)
- Node 20+ and npm
- A browser wallet set to **devnet** (Phantom, Solflare, Backpack, ...) with a little devnet SOL
- The keeper key file `anchor/keeper/devnet-keeper-keypair.json` (it is gitignored; it exists on the
  machine that ran the deploy. Generate and fund a new one with `anchor/scripts/deploy_devnet.sh` if not.)

### A. Start the backend (keeper, in Docker)

```bash
cd anchor
cp .env.devnet.example .env                                    # devnet settings
./scripts/docker_load_key.sh keeper/devnet-keeper-keypair.json # one-time: key -> Docker volume (stdin, not baked in)
docker compose up -d --build                                   # builds the image (~3 min first time) and starts it
docker compose logs -f keeper                                  # watch it
```

You should see `scan: N plan(s) for keeper` every few seconds. The keeper restarts automatically
(`restart: unless-stopped`). Stop it with `docker compose down` (state in the `keeper-state` volume is kept).

Useful lines in the log:
- `due, running preflight`, then `sweep sent <sig>`: a plan was executed.
- `skipped: hard guard: finalized clock ... < deadline + margin`: normal; the keeper waits for the
  finalized clock (about 13 s behind real time on devnet) and a 10 s margin.
- `closed, burning nonce`, `burned nonce`: a cancelled plan's nonce account was reclaimed.

The keeper must be running for any plan to execute. The frontend only creates, signs and cancels.

### B. Start the frontend

```bash
cd frontend
cp .env.example .env      # already points at the devnet deployment above
npm install
npm run dev               # http://localhost:5173
```

### C. Use it

1. Open the page and **Connect** your wallet (devnet).
2. Enter the **beneficiary public key** and the **check-in period**, then **Create plan**. The wallet
   prompts **3 times**: create the nonce account, sign the sweep, create the plan. A grace period of
   10 s (`VITE_GRACE_SECONDS`) is added.
3. The page shows **Time left to sign**. Press **Sign** (1 prompt) to reset the timer.
4. If you stop signing, the countdown moves to the grace period, then the keeper executes the sweep
   and the page shows the plan as executed.
5. **Cancel plan** (1 prompt) closes the plan and frees your wallet to create another.

For a live demo, a 60 to 90 second period works well: the sweep lands roughly 15 to 25 s after the
deadline (finalized-clock lag plus the 10 s margin).

> The sweep moves the wallet's **entire** SOL balance, including what you need for fees. After a plan
> executes, that wallet is at 0 SOL and needs a top-up before it can cancel or create again.

### D. No UI: the CLI

```bash
cd anchor
export OWNER_KEYPAIR=/path/to/owner.json
cargo run -p kimida-client -- setup --keeper AsEvw9nKTdCjW3hK3RiJtwNb2rNwaf8EYZmyRABCFGBM \
    --beneficiary <PUBKEY> --interval 60 --grace 10 --rpc https://api.devnet.solana.com
cargo run -p kimida-client -- status      # also: check-in, update, close
```

## 5. Configuration

**Keeper (`anchor/.env`)**

| Variable | Meaning |
|----------|---------|
| `RPC_URL` | Solana RPC (public devnet works; a dedicated endpoint is safer for a live demo) |
| `KEEPER_EXPECTED_GENESIS_HASH` | refuse to run if the RPC is a different cluster |
| `KEEPER_KEYPAIR`, `KEEPER_STATE` | host paths; **overridden in Docker** to `/secrets/keeper.json` and `/data/keeper-state.json` |
| `KEEPER_SAFETY_MARGIN` | seconds past the deadline (finalized clock) before signing (REQ26) |
| `KEEPER_SCAN_EVERY` | seconds between scans in Docker (default 5) |

**Frontend (`frontend/.env`, `VITE_*`)**: `VITE_RPC_URL`, `VITE_PROGRAM_ID`, `VITE_KEEPER_PUBKEY`,
`VITE_GRACE_SECONDS`. The page and the keeper must target the same cluster, program and keeper key.

## 6. Tests

```bash
cd anchor && anchor build && cargo test --workspace      # ~2 min
cd frontend && npm run build                              # typecheck + bundle
```

| Suite | Count | Covers |
|-------|------:|--------|
| `programs/kimida/tests/plan_lifecycle.rs` (LiteSVM) | 33 | on-chain REQ01-REQ15, REQ17-REQ20, REQ25 |
| `client` unit tests | 5 | nonce account parsing (rejects legacy/uninitialized) |
| `keeper` unit tests | 8 | deadline logic, sweep-structure preflight |
| `keeper/tests/e2e_multi_user.rs` | 1 | real validator: four owners, interleaved create / check-in / sweep / cancel, exact balances (REQ16, REQ21-REQ24, REQ26) |

Browser-wallet behaviour is verified manually on devnet; `frontend/scripts/devnet-smoke.ts` drives the
same transaction code with a keypair in place of a wallet.

## 7. Known limitations and open questions

- **Demo timers on devnet** (see section 2). Production bounds are still placeholders.
- **Orphan nonces.** The keeper only learns a nonce exists by seeing its plan. A plan created and closed
  between two scans, or a nonce created and never used (for example the wallet rejects the last prompt),
  stays on-chain with its rent locked. Options are in `anchor/README.md`.
- **One plan per wallet, not per person.** Nothing limits one person using many wallets
  (`frontend/NOTES.md`).
- **Wallet compatibility.** A wallet that rewrites the sweep (for example by adding priority-fee
  instructions) makes it unexecutable; the page detects this and refuses to create the plan. Test each
  wallet you intend to support.
- **Keeper trust and key custody.** One keeper key, kept as a file in a Docker volume. It cannot
  redirect funds, but it can withhold execution or burn nonces. HSM/KMS and an N-of-M signer set are
  post-MVP.
- **Liveness.** If the keeper is down, nothing executes. The check-in period is only as reliable as the
  backend is available.
- **Sweeps the whole balance** (SOL only; no SPL tokens, no multiple beneficiaries).
- **Not audited.** Security review is required before any real funds.

## 8. Deploy / upgrade

```bash
cd anchor
DEMO_SHORT_TIMERS=1 ./scripts/deploy_devnet.sh     # builds, checks balance, deploys, creates + funds a keeper key
# upgrade in place (same program ID, same authority wallet):
anchor build -- --features short-timers
solana program deploy target/deploy/kimida.so --program-id target/deploy/kimida-keypair.json -u devnet
```

The deploy script refuses any cluster whose genesis hash is not devnet's. A larger binary than the
current 197,744 bytes needs `solana program extend <PROGRAM_ID> <extra-bytes> -u devnet` first.

## 9. Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| Plan shows "can execute at any moment" but nothing happens | keeper is not running (`docker compose ps`), or the plan's sweep is malformed (see keeper log `skipped: ...`). Press **Sign** to re-sign, or **Cancel plan** |
| "Your wallet modified the sweep transaction" | the wallet rewrote the transaction; the plan was not created (a nonce account may be left behind) |
| Cannot create a plan: account already in use | this wallet already has a plan; use **Cancel plan** first |
| `docker compose up` hangs in "Created" | Docker Desktop needs permission / a restart; the key is loaded through a volume, so no host folder mount is needed |
| Keeper logs `wrong cluster` | `RPC_URL` points at a different cluster than `KEEPER_EXPECTED_GENESIS_HASH` |
| Wallet at 0 SOL after an execution | expected: the sweep took the entire balance; top up to cancel or start a new plan |
