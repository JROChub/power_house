#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

JULIAN_BIN="${JULIAN_BIN:-$ROOT_DIR/target/release/julian}"
NODES="${NODES:-10}"
BASE_PORT="${BASE_PORT:-7001}"
BLOB_BASE_PORT="${BLOB_BASE_PORT:-8181}"
METRICS_BASE_PORT="${METRICS_BASE_PORT:-9100}"
TOKIO_THREADS="${TOKIO_THREADS:-4}"
LISTEN_HOST="${LISTEN_HOST:-127.0.0.1}"
QUORUM="${QUORUM:-7}"
ATT_Q="${ATT_Q:-3}"
BOOTNODES="${BOOTNODES:-}"
ANCHOR_TOPIC="${ANCHOR_TOPIC:-}"
GOSSIP_SHARD="${GOSSIP_SHARD:-}"
BFT="${BFT:-0}"
BFT_ROUND_MS="${BFT_ROUND_MS:-}"
DETACH="${DETACH:-0}"
BLOB_MAX_CONCURRENCY="${BLOB_MAX_CONCURRENCY:-}"
BLOB_REQUEST_TIMEOUT_MS="${BLOB_REQUEST_TIMEOUT_MS:-}"

if [[ ! -x "$JULIAN_BIN" ]]; then
  echo "julian binary not found at $JULIAN_BIN; building release with net feature..."
  cargo +stable build --release --features net
fi

if [[ "$NODES" -lt 1 ]]; then
  echo "NODES must be >= 1"
  exit 1
fi

if [[ "$QUORUM" -gt "$NODES" ]]; then
  QUORUM="$NODES"
fi
if [[ "$ATT_Q" -gt "$QUORUM" ]]; then
  ATT_Q="$QUORUM"
fi

LOG_DIR="$ROOT_DIR/logs"
PIDS_FILE="$LOG_DIR/scale_pids.txt"
NODES_FILE="$LOG_DIR/scale_nodes.json"
mkdir -p "$LOG_DIR"
rm -f "$PIDS_FILE" "$NODES_FILE"

wait_for_peer() {
  local log_file=$1
  local node_id=$2
  local tries=120
  local line=""
  for _ in $(seq 1 "$tries"); do
    line=$(grep -m1 "QSYS|mod=NET|evt=LISTEN|node=${node_id} " "$log_file" || true)
    if [[ -n "$line" ]]; then
      local peer
      local addr
      peer=$(echo "$line" | sed -n 's/.*peer=\([^ ]*\).*/\1/p')
      addr=$(echo "$line" | sed -n 's/.* addr=\([^ ]*\).*/\1/p')
      if [[ -n "$peer" && -n "$addr" ]]; then
        echo "${peer}|${addr}"
        return 0
      fi
    fi
    sleep 0.5
  done
  return 1
}

start_node() {
  local idx=$1
  local node_id="node$((idx + 1))"
  local listen_port=$((BASE_PORT + idx))
  local metrics_port=$((METRICS_BASE_PORT + idx))
  local blob_port=$((BLOB_BASE_PORT + idx))
  local log_dir="$LOG_DIR/scale_${node_id}"
  local blob_dir="$LOG_DIR/scale_blobs_${node_id}"
  local out_file="$LOG_DIR/scale_${node_id}.out"

  mkdir -p "$log_dir" "$blob_dir"

  local cmd=(
    "$JULIAN_BIN" net start
    --node-id "$node_id"
    --log-dir "$log_dir"
    --listen "/ip4/${LISTEN_HOST}/tcp/${listen_port}"
    --broadcast-interval 1000
    --quorum "$QUORUM"
    --attestation-quorum "$ATT_Q"
    --blob-dir "$blob_dir"
    --blob-listen "${LISTEN_HOST}:${blob_port}"
    --metrics ":${metrics_port}"
    --tokio-threads "$TOKIO_THREADS"
    --key "ed25519://scale-${node_id}"
  )

  if [[ -n "$ANCHOR_TOPIC" ]]; then
    cmd+=(--anchor-topic "$ANCHOR_TOPIC")
  elif [[ -n "$GOSSIP_SHARD" ]]; then
    cmd+=(--gossip-shard "$GOSSIP_SHARD")
  fi
  if [[ "$BFT" == "1" ]]; then
    cmd+=(--bft)
    if [[ -n "$BFT_ROUND_MS" ]]; then
      cmd+=(--bft-round-ms "$BFT_ROUND_MS")
    fi
  fi
  if [[ -n "$BLOB_MAX_CONCURRENCY" ]]; then
    cmd+=(--blob-max-concurrency "$BLOB_MAX_CONCURRENCY")
  fi
  if [[ -n "$BLOB_REQUEST_TIMEOUT_MS" ]]; then
    cmd+=(--blob-request-timeout-ms "$BLOB_REQUEST_TIMEOUT_MS")
  fi

  if [[ -n "$BOOTNODES" ]]; then
    cmd+=(--bootnodes "$BOOTNODES")
  fi

  if [[ "$DETACH" == "1" ]]; then
    nohup "${cmd[@]}" > "$out_file" 2>&1 &
  else
    "${cmd[@]}" > "$out_file" 2>&1 &
  fi
  echo $! >> "$PIDS_FILE"

  local info
  if ! info=$(wait_for_peer "$out_file" "$node_id"); then
    echo "failed to read peer id for ${node_id}; see ${out_file}"
    exit 1
  fi
  local peer_id="${info%%|*}"
  local listen_addr="${info##*|}"
  local multiaddr="${listen_addr}/p2p/${peer_id}"

  printf '%s|%s|%s|%s|%s|%s\n' \
    "$node_id" \
    "$peer_id" \
    "$listen_addr" \
    "$multiaddr" \
    "${LISTEN_HOST}:${metrics_port}" \
    "${LISTEN_HOST}:${blob_port}"
}

echo "starting ${NODES} local nodes (quorum=${QUORUM}, attestation_quorum=${ATT_Q})"

node_info=()
node_info+=( "$(start_node 0)" )

if [[ -z "$BOOTNODES" ]]; then
  local_bootnode_multiaddr="$(echo "${node_info[0]}" | cut -d'|' -f4)"
  BOOTNODES="$local_bootnode_multiaddr"
fi

for i in $(seq 1 $((NODES - 1))); do
  node_info+=( "$(start_node "$i")" )
done

{
  echo "["
  for idx in "${!node_info[@]}"; do
    IFS='|' read -r node_id peer_id listen_addr multiaddr metrics_addr blob_addr <<< "${node_info[$idx]}"
    printf '  {"node_id":"%s","peer_id":"%s","listen_addr":"%s","multiaddr":"%s","metrics":"%s","blob":"%s"}' \
      "$node_id" "$peer_id" "$listen_addr" "$multiaddr" "$metrics_addr" "$blob_addr"
    if [[ "$idx" -lt $(( ${#node_info[@]} - 1 )) ]]; then
      echo ","
    else
      echo
    fi
  done
  echo "]"
} > "$NODES_FILE"

echo "nodes written to $NODES_FILE"
echo "pids written to $PIDS_FILE"
