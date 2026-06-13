#!/usr/bin/env bash
set -euo pipefail

CONTEXT="${DOCTL_CONTEXT:-mfenx-production}"
REGIONS="${DO_REGIONS:-nyc3,sfo3,ams3}"
SIZE="${DO_SIZE:-s-2vcpu-2gb}"
RPC_HOST="${RPC_HOST:-rpc.mfenx.com}"

for command in doctl python3 dig; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "BLOCKED: missing required command: $command" >&2
    exit 2
  }
done

doctl_cmd() {
  doctl --context "$CONTEXT" "$@"
}

account_json=$(doctl_cmd account get --output json 2>/dev/null) || {
  echo "BLOCKED: DigitalOcean context '$CONTEXT' is not authenticated." >&2
  echo "Revoke any exposed token, then run: doctl auth init --context $CONTEXT" >&2
  exit 2
}

python3 - "$account_json" <<'PY'
import json
import sys

raw = json.loads(sys.argv[1])
account = raw[0] if isinstance(raw, list) else raw
if account.get("status") != "active":
    raise SystemExit("BLOCKED: DigitalOcean account status is not active")
if not account.get("email_verified", False):
    raise SystemExit("BLOCKED: DigitalOcean account email is not verified")
if int(account.get("droplet_limit", 0)) < 3:
    raise SystemExit("BLOCKED: DigitalOcean Droplet limit is below three")
print(
    "account_status=active "
    f"droplet_limit={account.get('droplet_limit')} "
    f"email={account.get('email', 'unknown')}"
)
PY

regions_json=$(doctl_cmd compute region list --output json)
sizes_json=$(doctl_cmd compute size list --output json)
ssh_keys_json=$(doctl_cmd compute ssh-key list --output json)

python3 - "$regions_json" "$sizes_json" "$ssh_keys_json" "$REGIONS" "$SIZE" <<'PY'
import json
import sys

regions = {item["slug"]: item for item in json.loads(sys.argv[1])}
sizes = {item["slug"]: item for item in json.loads(sys.argv[2])}
ssh_keys = json.loads(sys.argv[3])
required_regions = sys.argv[4].split(",")
size_slug = sys.argv[5]

missing = [
    region
    for region in required_regions
    if region not in regions or not regions[region].get("available", False)
]
if missing:
    raise SystemExit(f"BLOCKED: unavailable DigitalOcean regions: {','.join(missing)}")
if size_slug not in sizes or not sizes[size_slug].get("available", False):
    raise SystemExit(f"BLOCKED: unavailable DigitalOcean size: {size_slug}")
if not ssh_keys:
    raise SystemExit("BLOCKED: no SSH key is registered in the DigitalOcean account")

size = sizes[size_slug]
print(
    f"regions={','.join(required_regions)} "
    f"size={size_slug} "
    f"monthly_per_droplet={size.get('price_monthly')} "
    f"ssh_keys={len(ssh_keys)}"
)
PY

projects=$(doctl_cmd projects list --output json)
droplets=$(doctl_cmd compute droplet list --output json)
firewalls=$(doctl_cmd compute firewall list --output json)
load_balancers=$(doctl_cmd compute load-balancer list --output json)
addresses=$(dig +short A "$RPC_HOST" | paste -sd, -)

python3 - "$projects" "$droplets" "$firewalls" "$load_balancers" <<'PY'
import json
import sys

labels = ("projects", "droplets", "firewalls", "load_balancers")
for label, raw in zip(labels, sys.argv[1:]):
    print(f"{label}={len(json.loads(raw))}")
PY

echo "rpc_host=$RPC_HOST"
echo "rpc_dns=${addresses:-not-configured}"
echo "LAX MFENX RPC infrastructure preflight: READY"
