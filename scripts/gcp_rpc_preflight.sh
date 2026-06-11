#!/usr/bin/env bash
set -euo pipefail

PROJECT="${GCP_PROJECT:-mfenx-485623}"
RPC_HOST="${RPC_HOST:-rpc.mfenx.com}"
REQUIRED_APIS=(
  compute.googleapis.com
  logging.googleapis.com
  monitoring.googleapis.com
)

for command in gcloud dig; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "missing required command: $command" >&2
    exit 1
  }
done

account=$(gcloud auth list --filter=status:ACTIVE --format='value(account)' | head -1)
[[ -n "$account" ]] || {
  echo "BLOCKED: no active gcloud account" >&2
  exit 2
}

billing=$(
  gcloud billing projects describe "$PROJECT" \
    --format='value(billingEnabled)' 2>/dev/null || true
)
if [[ "$billing" != "True" ]]; then
  echo "BLOCKED: billing is disabled for GCP project $PROJECT" >&2
  echo "Attach a billing account before provisioning always-on validators." >&2
  exit 2
fi

enabled_apis=$(
  gcloud services list --enabled --project="$PROJECT" \
    --format='value(config.name)'
)
missing=()
for api in "${REQUIRED_APIS[@]}"; do
  grep -Fxq "$api" <<<"$enabled_apis" || missing+=("$api")
done
if [[ ${#missing[@]} -gt 0 ]]; then
  echo "BLOCKED: required APIs are not enabled in $PROJECT:" >&2
  printf '  %s\n' "${missing[@]}" >&2
  echo "Enable them with:" >&2
  printf '  gcloud services enable --project=%q' "$PROJECT" >&2
  printf ' %q' "${missing[@]}" >&2
  printf '\n' >&2
  exit 2
fi

addresses=$(dig +short A "$RPC_HOST" | paste -sd, -)
if [[ -n "$addresses" ]]; then
  dns_status="$addresses"
else
  dns_status="not-configured"
fi

cat <<EOF
GCP RPC preflight: READY
account=$account
project=$PROJECT
billing=$billing
rpc_host=$RPC_HOST
rpc_dns=$dns_status

DNS may remain unconfigured until the HTTPS load balancer reserves its global IP.
EOF
