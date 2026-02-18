#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

CARGO_BIN=${CARGO_BIN:-cargo}
JULIAN_BIN=${JULIAN_BIN:-$ROOT_DIR/target/release/julian}

LOG_DIR_A="${FAULT_LOG_DIR_A:-./logs/fault_nodeA}"
LOG_DIR_B="${FAULT_LOG_DIR_B:-./logs/fault_nodeB}"
BLOB_DIR="${FAULT_BLOB_DIR:-./logs/fault_blob}"

random_port() {
  python - <<'PY'
import socket
s=socket.socket()
s.bind(('',0))
print(s.getsockname()[1])
s.close()
PY
}

PORT_A=${FAULT_PORT_A:-$(random_port)}
PORT_B=${FAULT_PORT_B:-$(random_port)}
BLOB_PORT=${FAULT_BLOB_PORT:-$(random_port)}
BLOB_LISTEN_A="${FAULT_BLOB_LISTEN_A:-127.0.0.1:${BLOB_PORT}}"

cleanup() {
  [[ -n "${PID_A:-}" ]] && kill "$PID_A" 2>/dev/null || true
  [[ -n "${PID_B:-}" ]] && kill "$PID_B" 2>/dev/null || true
  wait "$PID_A" 2>/dev/null || true
  wait "$PID_B" 2>/dev/null || true
}

trap cleanup EXIT

rm -rf "$LOG_DIR_A" "$LOG_DIR_B" "$BLOB_DIR"
mkdir -p "$LOG_DIR_A" "$LOG_DIR_B" "$BLOB_DIR"

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
  if [[ -x "$JULIAN_BIN" ]]; then
    stdbuf -oL -eL "$JULIAN_BIN" net start \
      --node-id "$node_id" \
      --log-dir "$log_dir" \
      --listen "/ip4/127.0.0.1/tcp/$port" \
      --broadcast-interval 1000 \
      --quorum 2 \
      --attestation-quorum 1 \
      "${blob_args[@]}" \
      --key "ed25519://fault-$node_id" \
      >"$out_file" 2>&1 &
  else
    stdbuf -oL -eL "$CARGO_BIN" run --features net --bin julian --quiet -- net start \
      --node-id "$node_id" \
      --log-dir "$log_dir" \
      --listen "/ip4/127.0.0.1/tcp/$port" \
      --broadcast-interval 1000 \
      --quorum 2 \
      --attestation-quorum 1 \
      "${blob_args[@]}" \
      --key "ed25519://fault-$node_id" \
      >"$out_file" 2>&1 &
  fi
  echo $!
}

TMP_A="$(mktemp)"
TMP_B="$(mktemp)"

PID_A=$(run_node nodeA "$PORT_A" "$LOG_DIR_A" "$TMP_A" "$BLOB_LISTEN_A")
sleep 2

if [[ -x "$JULIAN_BIN" ]]; then
  stdbuf -oL -eL "$JULIAN_BIN" net start \
    --node-id nodeB \
    --log-dir "$LOG_DIR_B" \
    --listen "/ip4/127.0.0.1/tcp/$PORT_B" \
    --bootstrap "/ip4/127.0.0.1/tcp/$PORT_A" \
    --broadcast-interval 1000 \
    --quorum 2 \
    --attestation-quorum 1 \
    --blob-dir "$BLOB_DIR" \
    --key "ed25519://fault-nodeB" \
    >"$TMP_B" 2>&1 &
else
  stdbuf -oL -eL "$CARGO_BIN" run --features net --bin julian --quiet -- net start \
    --node-id nodeB \
    --log-dir "$LOG_DIR_B" \
    --listen "/ip4/127.0.0.1/tcp/$PORT_B" \
    --bootstrap "/ip4/127.0.0.1/tcp/$PORT_A" \
    --broadcast-interval 1000 \
    --quorum 2 \
    --attestation-quorum 1 \
    --blob-dir "$BLOB_DIR" \
    --key "ed25519://fault-nodeB" \
    >"$TMP_B" 2>&1 &
fi
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

wait_for_port "$PORT_A" || { echo "port $PORT_A not listening"; tail -n 50 "$TMP_A" || true; exit 1; }
wait_for_port "$PORT_B" || { echo "port $PORT_B not listening"; tail -n 50 "$TMP_B" || true; exit 1; }
wait_for_port "$BLOB_PORT" || { echo "blob port $BLOB_PORT not listening"; tail -n 50 "$TMP_A" || true; exit 1; }

SAMPLE_BLOB="${FAULT_SAMPLE_BLOB:-$ROOT_DIR/sample.bin}"
if [[ ! -f "$SAMPLE_BLOB" ]]; then
  echo "missing sample blob: $SAMPLE_BLOB"
  exit 1
fi

FEE="${FAULT_FEE:-0}"
if ! SUBMIT_JSON=$(curl -sS -X POST "http://${BLOB_LISTEN_A}/submit_blob" \
  -H 'X-Namespace: default' \
  -H "X-Fee: ${FEE}" \
  --data-binary @"$SAMPLE_BLOB"); then
  echo "submit_blob failed"
  tail -n 50 "$TMP_A" || true
  exit 1
fi

if [[ -z "$SUBMIT_JSON" ]]; then
  echo "submit_blob empty response"
  tail -n 50 "$TMP_A" || true
  exit 1
fi

if ! HASH=$(python -c 'import json,sys; obj=json.loads(sys.stdin.read()); print(obj.get("hash",""))' <<<"$SUBMIT_JSON"); then
  echo "submit_blob invalid JSON: $SUBMIT_JSON"
  tail -n 50 "$TMP_A" || true
  exit 1
fi
if [[ -z "$HASH" ]]; then
  echo "submit_blob missing hash: $SUBMIT_JSON"
  tail -n 50 "$TMP_A" || true
  exit 1
fi

SHARE_PATH="$BLOB_DIR/default/$HASH/shares/0.share"
if [[ -f "$SHARE_PATH" ]]; then
  rm -f "$SHARE_PATH"
fi

curl -sS "http://${BLOB_LISTEN_A}/prove_storage/default/$HASH/0" >/dev/null || true

if [[ ! -f "$BLOB_DIR/evidence_outbox.jsonl" ]]; then
  echo "evidence_outbox.jsonl not found"
  exit 1
fi

grep -q "blob-missing" "$BLOB_DIR/evidence_outbox.jsonl" || {
  echo "missing blob-missing evidence"
  exit 1
}

cleanup
rm -f "$TMP_A" "$TMP_B"

printf 'fault_test: PASS\n'
