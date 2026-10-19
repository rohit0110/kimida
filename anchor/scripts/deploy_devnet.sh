#!/usr/bin/env bash
# Deploy the program to DEVNET, create a devnet keeper key and fund it. Run from anchor/.
#
#   ./scripts/deploy_devnet.sh                      # production bounds (1-day min interval)
#   DEMO_SHORT_TIMERS=1 ./scripts/deploy_devnet.sh  # seconds-scale timers, so a demo can finish live
#
# Refuses to run against anything but devnet. Never use DEMO_SHORT_TIMERS for a mainnet build.
set -euo pipefail
cd "$(dirname "$0")/.."

URL=${RPC_URL:-https://api.devnet.solana.com}
WALLET=${DEPLOYER_KEYPAIR:-$HOME/.config/solana/id.json}
DEVNET_GENESIS=EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
KEEPER_KEY=keeper/devnet-keeper-keypair.json     # gitignored (*-keypair.json)
KEEPER_FUND_SOL=${KEEPER_FUND_SOL:-1}

[ "$(solana genesis-hash -u "$URL")" = "$DEVNET_GENESIS" ] || { echo "RPC $URL is not devnet; refusing"; exit 1; }

FEATURES=()
if [ "${DEMO_SHORT_TIMERS:-0}" = 1 ]; then
  FEATURES=(--features short-timers)
  echo "!! DEMO BUILD: min interval 5s, min grace 1s. Devnet demo only."
fi

echo "== build"
anchor build -- ${FEATURES[@]+"${FEATURES[@]}"}
SO=target/deploy/kimida.so
PROGRAM_ID=$(solana address -k target/deploy/kimida-keypair.json)
SIZE=$(stat -f%z "$SO" 2>/dev/null || stat -c%s "$SO")

echo "== balance check"
BAL=$(solana balance "$(solana address -k "$WALLET")" -u "$URL" | awk '{print $1}')
echo "deployer $(solana address -k "$WALLET") has $BAL SOL on devnet"
# peak need ~= buffer rent + program-data rent. Default programdata is 2x the .so (upgrade headroom).
MAXLEN=()
if awk "BEGIN{exit !($BAL >= 3.3)}"; then
  echo "enough for a default (2x, upgradeable-headroom) deploy"
elif awk "BEGIN{exit !($BAL >= 2.3)}"; then
  MAXLEN=(--max-len "$SIZE")
  echo "!! tight: deploying with --max-len $SIZE (an upgrade can't be larger than this build)"
else
  echo "need at least ~2.3 SOL (3.3 recommended); fund the deployer and re-run"; exit 1
fi

echo "== deploy"
solana program deploy "$SO" --program-id target/deploy/kimida-keypair.json \
  --keypair "$WALLET" -u "$URL" ${MAXLEN[@]+"${MAXLEN[@]}"}

echo "== keeper key"
if [ ! -f "$KEEPER_KEY" ]; then
  cargo run -q -p kimida-keeper -- --keypair "$KEEPER_KEY" keygen
fi
KEEPER=$(solana address -k "$KEEPER_KEY")
solana transfer "$KEEPER" "$KEEPER_FUND_SOL" --keypair "$WALLET" -u "$URL" --allow-unfunded-recipient

cat <<MSG

== done. Put this in anchor/.env (gitignored):
RPC_URL=$URL
KEEPER_EXPECTED_GENESIS_HASH=$DEVNET_GENESIS
KEEPER_KEYPAIR=$KEEPER_KEY
KEEPER_STATE=keeper/devnet-keeper-state.json
KEEPER_SAFETY_MARGIN=${KEEPER_SAFETY_MARGIN:-10}

program id : $PROGRAM_ID
keeper     : $KEEPER
MSG
