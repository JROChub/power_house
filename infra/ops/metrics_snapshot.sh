#!/usr/bin/env bash
set -euo pipefail

METRICS_URL=${PH_METRICS_URL:-http://127.0.0.1:9100/metrics}
OUT_DIR=${PH_METRICS_OUT_DIR:-/var/log/powerhouse}
NODE_ID=${PH_NODE_ID:-node}

mkdir -p "$OUT_DIR"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT="$OUT_DIR/metrics-${NODE_ID}.jsonl"

BODY=$(curl -s "$METRICS_URL" || true)
if [[ -z "$BODY" ]]; then
  exit 1
fi

extract() {
  local key="$1"
  echo "$BODY" | awk -v k="$key" '$1==k {print $2; exit 0}'
}

anchors_received=$(extract anchors_received_total)
anchors_verified=$(extract anchors_verified_total)
finality_events=$(extract finality_events_total)
invalid_envelopes=$(extract invalid_envelopes_total)
gossipsub_rejects=$(extract gossipsub_rejects_total)

printf '{"ts":"%s","node":"%s","anchors_received":%s,"anchors_verified":%s,"finality_events":%s,"invalid_envelopes":%s,"gossipsub_rejects":%s}\n' \
  "$STAMP" "$NODE_ID" "${anchors_received:-0}" "${anchors_verified:-0}" "${finality_events:-0}" "${invalid_envelopes:-0}" "${gossipsub_rejects:-0}" >> "$OUT"
