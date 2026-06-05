#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

for command in curl jq; do
  command -v "$command" >/dev/null || {
    echo "missing required command: $command" >&2
    exit 1
  }
done

if [[ -n "${JULIAN_BIN:-}" ]]; then
  [[ -x "$JULIAN_BIN" ]] || {
    echo "JULIAN_BIN is not executable: $JULIAN_BIN" >&2
    exit 1
  }
else
  cargo build --features net --bin julian
  JULIAN_BIN="$ROOT_DIR/target/debug/julian"
fi

P2P_BASE_PORT="${P2P_BASE_PORT:-17601}"
RPC_BASE_PORT="${RPC_BASE_PORT:-18645}"
WORK_DIR="$(mktemp -d)"

cleanup() {
  for pid_file in "$WORK_DIR"/*.pid; do
    [[ -f "$pid_file" ]] || continue
    kill "$(cat "$pid_file")" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

cat >"$WORK_DIR/policy.json" <<'JSON'
{
  "backend": "static",
  "allowlist": [
    "gIFdYcN4MVLieYSow0mkqFoPntyI1Jl1WIntgaHQZ5U=",
    "FmWXfDF/CHrTrmRp9e0fmZV0eNfAA8Pu2EbiG0rgJJk=",
    "VGvK1rRmPbiJK0EuhQg39gB+foKbqbbnYOlUGaCwtkM="
  ]
}
JSON

for node in 1 2 3; do
  mkdir -p "$WORK_DIR/node${node}/logs" "$WORK_DIR/node${node}/blob"
  cat >"$WORK_DIR/node${node}/blob/stake_registry.json" <<'JSON'
{
  "accounts": {
    "0x4a62316623ad457f02cdc5d997ded67a383ec569": {
      "balance": 5,
      "stake": 0,
      "slashed": false
    }
  }
}
JSON
done

rpc_call() {
  local port=$1
  local method=$2
  local params=$3
  curl -fsS \
    -H 'content-type: application/json' \
    --data "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"${method}\",\"params\":${params}}" \
    "http://127.0.0.1:${port}"
}

wait_for_rpc() {
  local port=$1
  for _ in $(seq 1 100); do
    curl -fsS "http://127.0.0.1:${port}/healthz" >/dev/null 2>&1 && return 0
    sleep 0.1
  done
  return 1
}

start_node() {
  local node=$1
  local seed=$2
  local bootstrap=${3:-}
  local p2p_port=$((P2P_BASE_PORT + node - 1))
  local rpc_port=$((RPC_BASE_PORT + node - 1))
  local args=(
    "$JULIAN_BIN" net start
    --node-id "cluster-${node}"
    --log-dir "$WORK_DIR/node${node}/logs"
    --blob-dir "$WORK_DIR/node${node}/blob"
    --listen "/ip4/127.0.0.1/tcp/${p2p_port}"
    --key "ed25519://${seed}"
    --policy "$WORK_DIR/policy.json"
    --quorum 2
    --broadcast-interval 250
    --evm-chain-id 177155
    --evm-rpc-listen "127.0.0.1:${rpc_port}"
  )
  if [[ -n "$bootstrap" ]]; then
    args+=(--bootstrap "$bootstrap")
  fi
  "${args[@]}" >"$WORK_DIR/node${node}.out" 2>&1 &
  echo $! >"$WORK_DIR/node${node}.pid"
}

start_node 1 cluster-a
for _ in $(seq 1 100); do
  grep -q 'QSYS|mod=NET|evt=LISTEN|node=cluster-1 ' "$WORK_DIR/node1.out" && break
  sleep 0.1
done
PEER_ID="$(sed -n 's/.*peer=\([^ ]*\).*/\1/p' "$WORK_DIR/node1.out" | head -1)"
[[ -n "$PEER_ID" ]] || {
  echo "failed to discover node 1 peer ID" >&2
  exit 1
}
BOOTSTRAP="/ip4/127.0.0.1/tcp/${P2P_BASE_PORT}/p2p/${PEER_ID}"
start_node 2 cluster-b "$BOOTSTRAP"

for node in 1 2; do
  wait_for_rpc "$((RPC_BASE_PORT + node - 1))" || {
    echo "node ${node} RPC failed to start" >&2
    tail -n 50 "$WORK_DIR/node${node}.out" >&2
    exit 1
  }
done
sleep 3

RAW_TRANSACTION="0x02f8758302b403808405f5e100843b9aca00825208940909090909090909090909090909090909090909881bc16d674ec8000080c080a08a3f8f6b9385705c665018550033c3c137adef9169eba72e8e6f44e02770bf4ca056ea776c9b6d9d2f9add3b3140e3a3adadcb34b7865152c370d24774d2d881b2"
TX_HASH="0x912aa4388b5593cf050822c5c7181e90aa041c09169fa6d262e8331c16fb19f0"
SENDER="0x4a62316623ad457f02cdc5d997ded67a383ec569"
RECIPIENT="0x0909090909090909090909090909090909090909"

SUBMITTED="$(
  rpc_call "$RPC_BASE_PORT" eth_sendRawTransaction "[\"${RAW_TRANSACTION}\"]" |
    jq -r '.result'
)"
[[ "$SUBMITTED" == "$TX_HASH" ]]

for _ in $(seq 1 120); do
  finalized=1
  for node in 1 2; do
    height="$(
      rpc_call "$((RPC_BASE_PORT + node - 1))" eth_blockNumber '[]' |
        jq -r '.result'
    )"
    [[ "$height" == "0x1" ]] || finalized=0
  done
  [[ "$finalized" == "1" ]] && break
  sleep 0.25
done

start_node 3 cluster-c "$BOOTSTRAP"
wait_for_rpc "$((RPC_BASE_PORT + 2))" || {
  echo "node 3 RPC failed to start" >&2
  tail -n 50 "$WORK_DIR/node3.out" >&2
  exit 1
}
for _ in $(seq 1 120); do
  height="$(
    rpc_call "$((RPC_BASE_PORT + 2))" eth_blockNumber '[]' |
      jq -r '.result'
  )"
  [[ "$height" == "0x1" ]] && break
  sleep 0.25
done

declare -a hashes roots
for node in 1 2 3; do
  port=$((RPC_BASE_PORT + node - 1))
  block="$(rpc_call "$port" eth_getBlockByNumber '["latest",false]')"
  receipt="$(rpc_call "$port" eth_getTransactionReceipt "[\"${TX_HASH}\"]")"
  sender_balance="$(rpc_call "$port" eth_getBalance "[\"${SENDER}\",\"latest\"]" | jq -r '.result')"
  recipient_balance="$(
    rpc_call "$port" eth_getBalance "[\"${RECIPIENT}\",\"latest\"]" |
      jq -r '.result'
  )"
  hashes+=("$(jq -r '.result.hash' <<<"$block")")
  roots+=("$(jq -r '.result.stateRoot' <<<"$block")")
  [[ "$(jq -r '.result.blockNumber' <<<"$receipt")" == "0x1" ]]
  [[ "$(jq -r '.result.status' <<<"$receipt")" == "0x1" ]]
  [[ "$sender_balance" == "0x29a2241af62c0000" ]]
  [[ "$recipient_balance" == "0x1bc16d674ec80000" ]]
done

[[ "${hashes[0]}" == "${hashes[1]}" && "${hashes[1]}" == "${hashes[2]}" ]]
[[ "${roots[0]}" == "${roots[1]}" && "${roots[1]}" == "${roots[2]}" ]]

if grep -E 'unknown proposal|does not extend the finalized tip' "$WORK_DIR"/*.out; then
  echo "native-chain message ordering regression detected" >&2
  exit 1
fi

echo "native_rpc_cluster: PASS height=1 hash=${hashes[0]} state_root=${roots[0]}"
