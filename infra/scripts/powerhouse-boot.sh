#!/usr/bin/env bash
set -euo pipefail

required() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "Missing required env: $name" >&2
    exit 1
  fi
}

required PH_NODE_ID
required PH_LOG_DIR
required PH_LISTEN
required PH_KEY

ARGS=(
  --node-id "$PH_NODE_ID"
  --log-dir "$PH_LOG_DIR"
  --listen "$PH_LISTEN"
  --key "$PH_KEY"
)

if [[ -n "${PH_BOOTSTRAPS:-}" ]]; then
  for addr in $PH_BOOTSTRAPS; do
    ARGS+=(--bootstrap "$addr")
  done
fi

if [[ -n "${PH_BROADCAST_INTERVAL:-}" ]]; then
  ARGS+=(--broadcast-interval "$PH_BROADCAST_INTERVAL")
fi

if [[ -n "${PH_QUORUM:-}" ]]; then
  ARGS+=(--quorum "$PH_QUORUM")
fi

if [[ -n "${PH_CHECKPOINT_INTERVAL:-}" ]]; then
  ARGS+=(--checkpoint-interval "$PH_CHECKPOINT_INTERVAL")
fi

if [[ -n "${PH_POLICY:-}" ]]; then
  ARGS+=(--policy "$PH_POLICY")
fi

if [[ -n "${PH_POLICY_ALLOWLIST:-}" ]]; then
  ARGS+=(--policy-allowlist "$PH_POLICY_ALLOWLIST")
fi

if [[ -n "${PH_METRICS_ADDR:-}" ]]; then
  ARGS+=(--metrics "$PH_METRICS_ADDR")
fi

if [[ -n "${PH_BLOB_DIR:-}" ]]; then
  ARGS+=(--blob-dir "$PH_BLOB_DIR")
fi

if [[ -n "${PH_BLOB_LISTEN:-}" ]]; then
  ARGS+=(--blob-listen "$PH_BLOB_LISTEN")
fi

if [[ -n "${PH_BLOB_POLICY:-}" ]]; then
  ARGS+=(--blob-policy "$PH_BLOB_POLICY")
fi

if [[ -n "${PH_BLOB_AUTH_TOKEN:-}" ]]; then
  ARGS+=(--blob-auth-token "$PH_BLOB_AUTH_TOKEN")
fi

if [[ -n "${PH_BLOB_MAX_CONCURRENCY:-}" ]]; then
  ARGS+=(--blob-max-concurrency "$PH_BLOB_MAX_CONCURRENCY")
fi

if [[ -n "${PH_BLOB_REQUEST_TIMEOUT_MS:-}" ]]; then
  ARGS+=(--blob-request-timeout-ms "$PH_BLOB_REQUEST_TIMEOUT_MS")
fi

if [[ -n "${PH_MAX_BLOB_BYTES:-}" ]]; then
  ARGS+=(--max-blob-bytes "$PH_MAX_BLOB_BYTES")
fi

if [[ -n "${PH_BLOB_RETENTION_DAYS:-}" ]]; then
  ARGS+=(--blob-retention-days "$PH_BLOB_RETENTION_DAYS")
fi

if [[ -n "${PH_ATTESTATION_QUORUM:-}" ]]; then
  ARGS+=(--attestation-quorum "$PH_ATTESTATION_QUORUM")
fi

exec /usr/local/bin/julian net start "${ARGS[@]}"
