#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

CARGO_BIN=${CARGO_BIN:-cargo}
RUSTFLAGS=${RUSTFLAGS:-}

LOG_DIR_A="${SMOKE_LOG_DIR_A:-./logs/smoke_nodeA}"
LOG_DIR_B="${SMOKE_LOG_DIR_B:-./logs/smoke_nodeB}"
PORT_A=7211
PORT_B=7212

cleanup() {
  [[ -n "${PID_A:-}" ]] && kill "$PID_A" 2>/dev/null || true
  [[ -n "${PID_B:-}" ]] && kill "$PID_B" 2>/dev/null || true
  wait "$PID_A" 2>/dev/null || true
  wait "$PID_B" 2>/dev/null || true
}

trap cleanup EXIT

rm -rf "$LOG_DIR_A" "$LOG_DIR_B"
mkdir -p "$LOG_DIR_A" "$LOG_DIR_B"

run_node() {
  local node_id=$1
  local port=$2
  local log_dir=$3
  local out_file=$4
  stdbuf -oL -eL "$CARGO_BIN" run --features net --bin julian --quiet -- net start \
    --node-id "$node_id" \
    --log-dir "$log_dir" \
    --listen "/ip4/127.0.0.1/tcp/$port" \
    --broadcast-interval 1000 \
    --quorum 2 \
    --key "ed25519://smoke-$node_id" \
    >"$out_file" 2>&1 &
  echo $!
}

TMP_A="$(mktemp)"
TMP_B="$(mktemp)"

PID_A=$(run_node nodeA "$PORT_A" "$LOG_DIR_A" "$TMP_A")
sleep 2

stdbuf -oL -eL "$CARGO_BIN" run --features net --bin julian --quiet -- net start \
  --node-id nodeB \
  --log-dir "$LOG_DIR_B" \
  --listen "/ip4/127.0.0.1/tcp/$PORT_B" \
  --bootstrap "/ip4/127.0.0.1/tcp/$PORT_A" \
  --broadcast-interval 1000 \
  --quorum 2 \
  --key "ed25519://smoke-nodeB" \
  >"$TMP_B" 2>&1 &
PID_B=$!

sleep 6

cleanup

check_output() {
  local file=$1
  grep -q "listening on" "$file" || { echo "missing listen confirmation in $file"; return 1; }
  grep -q "broadcasted local anchor" "$file" || { echo "missing broadcast in $file"; return 1; }
  grep -q "finality reached" "$file" || { echo "missing finality in $file"; return 1; }
}

check_output "$TMP_A"
check_output "$TMP_B"

rm -f "$TMP_A" "$TMP_B"

printf 'smoke_net: PASS\n'
