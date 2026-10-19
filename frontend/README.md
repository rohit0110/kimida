# KimiDa frontend

Full project guide (devnet deployment, backend in Docker, demo steps): `../README.md`.

Single page: connect a wallet, enter a beneficiary and a check-in period, create the plan. Once the
wallet has a plan the page shows the time left to sign and a **Sign** button.

Stack: Vite, React, TypeScript, `@solana/wallet-adapter` (Wallet Standard wallets are auto-detected).
No Anchor TS client: `src/kimida.ts` encodes the instructions directly, using the discriminators in
`../anchor/target/idl/kimida.json`.

## Run

```bash
cd frontend
cp .env.example .env     # already points at the devnet demo deployment
npm install
npm run dev              # http://localhost:5173
npm run build            # typecheck + production bundle
```

## Config (`.env`, `VITE_*` only)

| Variable | Meaning |
|----------|---------|
| `VITE_RPC_URL` | Solana RPC (default devnet) |
| `VITE_PROGRAM_ID` | the deployed KimiDa program |
| `VITE_KEEPER_PUBKEY` | keeper that becomes nonce authority and co-signer of plans made here. Must match the running keeper |
| `VITE_GRACE_SECONDS` | grace applied to every plan created from the page |

The page and the keeper must point at the same cluster and program. The keeper must be running
(`kimida-keeper run`, see `../anchor/README.md`) for plans to execute.

## What the buttons do

| Action | On-chain | Wallet prompts |
|--------|----------|----------------|
| Create plan | create nonce account (authority = keeper) -> sign partial sweep -> `initialize_plan` | 3 |
| Sign | `check_in` (resets the timer), resending the stored partial sweep | 1 |
| Cancel plan / Start a new plan | `close_plan` (rent back to owner; the keeper then burns the nonce) | 1 |

"Time left to sign" counts down to `last_checkin + interval`, using the cluster's `Clock` sysvar
(not the browser clock). After that the grace period counts down, then the keeper may execute.

## Verifying without a browser wallet

```bash
npm run smoke -- <owner-keypair.json> <beneficiary-pubkey> 40   # create + check in via src/kimida.ts
npx tsx scripts/read-plan.ts <owner-pubkey>                      # decode a plan
```

See `NOTES.md` for open questions (including the one-plan-per-wallet limit).
