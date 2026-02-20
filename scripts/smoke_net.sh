#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

CARGO_BIN=${CARGO_BIN:-cargo}
RUSTFLAGS=${RUSTFLAGS:-}

LOG_DIR_A="${SMOKE_LOG_DIR_A:-./logs/smoke_nodeA}"
LOG_DIR_B="${SMOKE_LOG_DIR_B:-./logs/smoke_nodeB}"
BLOB_DIR="${SMOKE_BLOB_DIR:-./logs/smoke_blob}"
BLOB_LISTEN_A="${SMOKE_BLOB_LISTEN_A:-127.0.0.1:8891}"
PORT_A=7211
PORT_B=7212
WITH_MIGRATION=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-migration)
      WITH_MIGRATION=1
      shift
      ;;
    *)
      echo "unknown argument: $1"
      exit 1
      ;;
  esac
done

cleanup() {
  [[ -n "${PID_A:-}" ]] && kill "$PID_A" 2>/dev/null || true
  [[ -n "${PID_B:-}" ]] && kill "$PID_B" 2>/dev/null || true
  wait "$PID_A" 2>/dev/null || true
  wait "$PID_B" 2>/dev/null || true
}

trap cleanup EXIT

rm -rf "$LOG_DIR_A" "$LOG_DIR_B" "$BLOB_DIR"
mkdir -p "$LOG_DIR_A" "$LOG_DIR_B" "$BLOB_DIR"

if [[ "$WITH_MIGRATION" -eq 1 ]]; then
  MIGRATION_REGISTRY="$BLOB_DIR/migration_registry.json"
  MIGRATION_SNAPSHOT="$BLOB_DIR/migration_snapshot.json"
  MIGRATION_ANCHOR="$BLOB_DIR/migration_anchor.json"

  cat >"$MIGRATION_REGISTRY" <<'JSON'
{
  "accounts": {
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=": {
      "balance": 1000,
      "stake": 250,
      "slashed": false
    },
    "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=": {
      "balance": 900,
      "stake": 125,
      "slashed": false
    }
  }
}
JSON

  "$CARGO_BIN" run --features net --bin julian --quiet -- \
    stake snapshot --registry "$MIGRATION_REGISTRY" --height 1 --output "$MIGRATION_SNAPSHOT" >/dev/null

  "$CARGO_BIN" run --features net --bin julian --quiet -- \
    governance propose-migration \
      --snapshot-height 1 \
      --token-contract "0x0000000000000000000000000000000000000001" \
      --conversion-ratio 1 \
      --treasury-mint 0 \
      --log-dir "$LOG_DIR_A" \
      --node-id "smoke-migration" \
      --quorum 1 \
      --output "$MIGRATION_ANCHOR" >/dev/null
fi

run_node() {
  local node_id=$1
  local port=$2
  local log_dir=$3
  local out_file=$4
  local blob_listen=${5:-}
  local blob_args=()
  blob_args+=(--blob-dir "$BLOB_DIR")
  if [[ -n "$blob_listen" ]]; then
    blob_args+=(--blob-listen "$blob_listen")
  fi
  stdbuf -oL -eL "$CARGO_BIN" run --features net --bin julian --quiet -- net start \
    --node-id "$node_id" \
    --log-dir "$log_dir" \
    --listen "/ip4/127.0.0.1/tcp/$port" \
    --broadcast-interval 1000 \
    --quorum 2 \
    --attestation-quorum 1 \
    "${blob_args[@]}" \
    --key "ed25519://smoke-$node_id" \
    >"$out_file" 2>&1 &
  echo $!
}

TMP_A="$(mktemp)"
TMP_B="$(mktemp)"

PID_A=$(run_node nodeA "$PORT_A" "$LOG_DIR_A" "$TMP_A" "$BLOB_LISTEN_A")
sleep 2

stdbuf -oL -eL "$CARGO_BIN" run --features net --bin julian --quiet -- net start \
  --node-id nodeB \
  --log-dir "$LOG_DIR_B" \
  --listen "/ip4/127.0.0.1/tcp/$PORT_B" \
  --bootstrap "/ip4/127.0.0.1/tcp/$PORT_A" \
  --broadcast-interval 1000 \
  --quorum 2 \
  --attestation-quorum 1 \
  --blob-dir "$BLOB_DIR" \
  --key "ed25519://smoke-nodeB" \
  >"$TMP_B" 2>&1 &
PID_B=$!

wait_for_port() {
  local port=$1
  for _ in $(seq 1 120); do
    if ss -lnt 2>/dev/null | grep -q ":${port}"; then
      return 0
    fi
    sleep 0.5
  done
  return 1
}

wait_for_log() {
  local file=$1
  local pattern=$2
  for _ in $(seq 1 120); do
    if grep -q "$pattern" "$file"; then
      return 0
    fi
    sleep 0.5
  done
  return 1
}

wait_for_port "$PORT_A" || { echo "port $PORT_A not listening"; tail -n 50 "$TMP_A" || true; exit 1; }
wait_for_port "$PORT_B" || { echo "port $PORT_B not listening"; tail -n 50 "$TMP_B" || true; exit 1; }

SAMPLE_BLOB="${SMOKE_SAMPLE_BLOB:-$ROOT_DIR/sample.bin}"
if [[ ! -f "$SAMPLE_BLOB" ]]; then
  SAMPLE_BLOB="$BLOB_DIR/sample.bin"
  printf 'power_house smoke sample\n' >"$SAMPLE_BLOB"
fi
curl -sS -X POST "http://${BLOB_LISTEN_A}/submit_blob" \
  -H 'X-Namespace: default' \
  -H 'X-Fee: 1' \
  --data-binary @"$SAMPLE_BLOB" \
  >/dev/null

wait_for_log "$TMP_A" "QSYS|mod=ANCHOR|evt=BROADCAST" || { echo "missing broadcast in $TMP_A"; tail -n 50 "$TMP_A" || true; exit 1; }
wait_for_log "$TMP_B" "QSYS|mod=ANCHOR|evt=BROADCAST" || { echo "missing broadcast in $TMP_B"; tail -n 50 "$TMP_B" || true; exit 1; }
wait_for_log "$TMP_A" "QSYS|mod=QUORUM|evt=FINALIZED" || { echo "missing finality in $TMP_A"; tail -n 50 "$TMP_A" || true; exit 1; }
wait_for_log "$TMP_B" "QSYS|mod=QUORUM|evt=FINALIZED" || { echo "missing finality in $TMP_B"; tail -n 50 "$TMP_B" || true; exit 1; }

cleanup

rm -f "$TMP_A" "$TMP_B"

printf 'smoke_net: PASS\n'
