#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
OUTPUT=$(
  "$ROOT/scripts/provision_digitalocean_rpc.sh" \
    --ssh-key test-fingerprint \
    --ssh-cidr 203.0.113.10/32
)

grep -q 'MFENX DigitalOcean production plan' <<<"$OUTPUT"
grep -q 'nyc3,sfo3,ams3' <<<"$OUTPUT"
grep -q 's-2vcpu-2gb' <<<"$OUTPUT"
grep -q 'Plan only' <<<"$OUTPUT"
grep -q -- "--domains \"name:\$RPC_HOST is_managed:true\"" \
  "$ROOT/scripts/provision_digitalocean_rpc.sh"
grep -q "compute domain create \"\$DNS_ZONE\"" \
  "$ROOT/scripts/provision_digitalocean_rpc.sh"
grep -q "BLOCKED: \$DNS_ZONE is not delegated to DigitalOcean." \
  "$ROOT/scripts/provision_digitalocean_rpc.sh"

if "$ROOT/scripts/provision_digitalocean_rpc.sh" \
  --ssh-key test-fingerprint \
  --ssh-cidr not-a-cidr >/dev/null 2>&1; then
  echo "invalid SSH CIDR unexpectedly succeeded" >&2
  exit 1
fi

printf 'test_digitalocean_provision: PASS\n'
