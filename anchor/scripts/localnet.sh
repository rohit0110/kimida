#!/usr/bin/env bash
# Build the program with seconds-scale timers and start a local validator with it preloaded.
# Run from anchor/. Leaves the validator in the foreground.
set -euo pipefail
cd "$(dirname "$0")/.."
anchor build -- --features short-timers
PROGRAM_ID=$(solana address -k target/deploy/kimida-keypair.json)
echo "program id: $PROGRAM_ID"
exec solana-test-validator --reset --ledger test-ledger \
  --bpf-program "$PROGRAM_ID" target/deploy/kimida.so
