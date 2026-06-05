#!/usr/bin/env bash
set -euo pipefail

URL="${URL:-http://127.0.0.1:8181/submit_blob}"
BLOB="${BLOB:-./sample.bin}"
COUNT="${COUNT:-1000}"
RATE_PER_HOUR="${RATE_PER_HOUR:-0}"
NAMESPACE="${NAMESPACE:-default}"
FEE="${FEE:-0}"

GENERATED_BLOB=""
cleanup() {
  rm -f "$GENERATED_BLOB"
}
trap cleanup EXIT

if [[ ! -f "$BLOB" ]]; then
  GENERATED_BLOB="$(mktemp)"
  printf 'power_house load-test sample\n' >"$GENERATED_BLOB"
  BLOB="$GENERATED_BLOB"
fi

sleep_interval=0
interval_ns=0
if [[ "$RATE_PER_HOUR" -gt 0 ]]; then
  interval_ns=$(awk -v r="$RATE_PER_HOUR" 'BEGIN{printf "%.0f", (3600*1000000000)/r}')
fi

start_ns=$(date +%s%N)
success=0
for _ in $(seq 1 "$COUNT"); do
  req_start_ns=$(date +%s%N)
  if curl --fail-with-body -sS -X POST "$URL" \
    -H "X-Namespace: $NAMESPACE" \
    -H "X-Fee: $FEE" \
    --data-binary @"$BLOB" > /dev/null; then
    success=$((success+1))
  fi
  if [[ "$interval_ns" -gt 0 ]]; then
    req_end_ns=$(date +%s%N)
    elapsed_ns=$((req_end_ns - req_start_ns))
    if [[ "$elapsed_ns" -lt "$interval_ns" ]]; then
      sleep_ns=$((interval_ns - elapsed_ns))
      sleep_interval=$(awk -v ns="$sleep_ns" 'BEGIN{printf "%.6f", ns/1000000000}')
      sleep "$sleep_interval"
    fi
  fi
done
end_ns=$(date +%s%N)

python - <<PY
start_ns = int("$start_ns")
end_ns = int("$end_ns")
elapsed_s = (end_ns - start_ns) / 1e9
success = int("$success")
count = int("$COUNT")
print(f"elapsed_seconds={elapsed_s:.3f}")
print(f"success={success}/{count}")
if elapsed_s > 0:
    print(f"tps={success/elapsed_s:.2f}")
PY

if [[ "$success" -ne "$COUNT" ]]; then
  echo "load test failed: $((COUNT - success)) request(s) returned an error" >&2
  exit 1
fi
