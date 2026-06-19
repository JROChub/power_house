#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/provision_digitalocean_rpc.sh --ssh-key <id-or-fingerprint> \
    --ssh-cidr <public-ip/cidr> [--apply]

Without --apply, prints the production plan without creating billable resources.

Environment:
  DOCTL_CONTEXT  doctl context (default: mfenx-production)
  DO_REGIONS     three comma-separated regions (default: nyc3,sfo3,ams3)
  DO_SIZE        Droplet size (default: s-2vcpu-2gb)
  DO_IMAGE       Droplet image (default: ubuntu-24-04-x64)
  RPC_HOST       public RPC hostname (default: rpc.mfenx.com)
EOF
}

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
CONTEXT="${DOCTL_CONTEXT:-mfenx-production}"
REGIONS_CSV="${DO_REGIONS:-nyc3,sfo3,ams3}"
SIZE="${DO_SIZE:-s-2vcpu-2gb}"
IMAGE="${DO_IMAGE:-ubuntu-24-04-x64}"
RPC_HOST="${RPC_HOST:-rpc.mfenx.com}"
DNS_ZONE="$RPC_HOST"
PROJECT_NAME="MFENX Power-House"
TAG="mfenx-rpc-validator"
FIREWALL_NAME="mfenx-rpc-firewall"
LOAD_BALANCER_NAME="lax-mfenx-rpc"
LEGACY_LOAD_BALANCER_NAME="mfenx-rpc-global"
OUTPUT="${DO_INFRA_OUTPUT:-$ROOT/deployment/generated/digitalocean-infra.json}"
SSH_KEY=""
SSH_CIDR=""
APPLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ssh-key)
      SSH_KEY="${2:-}"
      shift 2
      ;;
    --ssh-cidr)
      SSH_CIDR="${2:-}"
      shift 2
      ;;
    --apply)
      APPLY=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

[[ -n "$SSH_KEY" ]] || {
  echo "--ssh-key is required" >&2
  exit 1
}
[[ -n "$SSH_CIDR" ]] || {
  echo "--ssh-cidr is required" >&2
  exit 1
}

python3 - "$SSH_CIDR" "$REGIONS_CSV" <<'PY'
import ipaddress
import sys

ipaddress.ip_network(sys.argv[1], strict=False)
regions = [item for item in sys.argv[2].split(",") if item]
if len(regions) != 3 or len(set(regions)) != 3:
    raise SystemExit("DO_REGIONS must contain exactly three distinct regions")
PY

IFS=',' read -r -a REGIONS <<<"$REGIONS_CSV"

cat <<EOF
MFENX DigitalOcean production plan
  project: $PROJECT_NAME
  regions: $REGIONS_CSV
  droplets: 3 x $SIZE ($IMAGE), weekly backups, monitoring
  firewall: SSH from $SSH_CIDR; validator P2P among validator tag; observer bootnode TCP 7002 public; HTTP from load balancer
  edge: global HTTPS load balancer and delegated DNS zone for $RPC_HOST
  estimated base: three Droplets + backups + \$15/month global load balancer
EOF

if [[ "$APPLY" != true ]]; then
  echo "Plan only. Re-run with --apply after reviewing expected charges."
  exit 0
fi

DOCTL_CONTEXT="$CONTEXT" DO_REGIONS="$REGIONS_CSV" DO_SIZE="$SIZE" \
  RPC_HOST="$RPC_HOST" "$ROOT/scripts/digitalocean_rpc_preflight.sh"

doctl_cmd() {
  doctl --context "$CONTEXT" "$@"
}

json_field_by_name() {
  local field=$1
  local name=$2
  python3 -c '
import json
import sys

field, name = sys.argv[1], sys.argv[2]
items = json.load(sys.stdin)
matches = [item for item in items if item.get("name") == name]
if len(matches) > 1:
    raise SystemExit(f"multiple resources named {name}")
if matches:
    value = matches[0].get(field)
    if isinstance(value, dict):
        value = value.get("slug") or value.get("name")
    if value is not None:
        print(value)
' "$field" "$name"
}

first_json_field() {
  local field=$1
  python3 -c '
import json
import sys

field = sys.argv[1]
data = json.load(sys.stdin)
if isinstance(data, list):
    if not data:
        raise SystemExit(f"DigitalOcean returned no resource while reading {field}")
    item = data[0]
else:
    item = data
if field not in item:
    raise SystemExit(
        f"DigitalOcean response is missing {field}: "
        + json.dumps(item, sort_keys=True)
    )
value = item[field]
if isinstance(value, dict):
    value = value.get("slug") or value.get("name")
print(value)
' "$field"
}

projects_json=$(doctl_cmd projects list --output json)
project_id=$(printf '%s' "$projects_json" | json_field_by_name id "$PROJECT_NAME")
if [[ -z "$project_id" ]]; then
  echo "-> creating DigitalOcean project"
  project_id=$(
    doctl_cmd projects create \
      --name "$PROJECT_NAME" \
      --description "MFENX Power-House production RPC validators" \
      --purpose "Blockchain" \
      --environment Production \
      --output json | first_json_field id
  )
fi

tags_json=$(doctl_cmd compute tag list --output json)
if ! python3 -c '
import json
import sys
raise SystemExit(0 if any(item.get("name") == sys.argv[1] for item in json.load(sys.stdin)) else 1)
' "$TAG" <<<"$tags_json"; then
  echo "-> creating validator tag"
  doctl_cmd compute tag create "$TAG" >/dev/null
fi

domains_json=$(doctl_cmd compute domain list --output json)
if ! python3 -c '
import json
import sys
raise SystemExit(0 if any(item.get("name") == sys.argv[1] for item in json.load(sys.stdin)) else 1)
' "$DNS_ZONE" <<<"$domains_json"; then
  echo "-> creating delegated DNS zone $DNS_ZONE"
  doctl_cmd compute domain create "$DNS_ZONE" >/dev/null
fi

command -v dig >/dev/null 2>&1 || {
  echo "BLOCKED: dig is required to verify public DNS delegation" >&2
  exit 2
}
public_nameservers=$(dig +short NS "$DNS_ZONE" | tr '[:upper:]' '[:lower:]')
for nameserver in ns1.digitalocean.com. ns2.digitalocean.com. ns3.digitalocean.com.; do
  if ! grep -qx "$nameserver" <<<"$public_nameservers"; then
    cat >&2 <<EOF
BLOCKED: $DNS_ZONE is not delegated to DigitalOcean.
Add these Namecheap NS records with host 'rpc', then rerun:
  ns1.digitalocean.com
  ns2.digitalocean.com
  ns3.digitalocean.com
EOF
    exit 2
  fi
done

firewalls_json=$(doctl_cmd compute firewall list --output json)
firewall_id=$(printf '%s' "$firewalls_json" | json_field_by_name id "$FIREWALL_NAME")
if [[ -z "$firewall_id" ]]; then
  echo "-> creating deny-by-default validator firewall"
  firewall_id=$(
    doctl_cmd compute firewall create \
      --name "$FIREWALL_NAME" \
      --tag-names "$TAG" \
      --inbound-rules \
        "protocol:tcp,ports:22,address:$SSH_CIDR protocol:tcp,ports:7001,tag:$TAG protocol:tcp,ports:7002,address:0.0.0.0/0 protocol:tcp,ports:9090,tag:$TAG protocol:tcp,ports:9100-9101,tag:$TAG" \
      --outbound-rules \
        "protocol:tcp,ports:all,address:0.0.0.0/0 protocol:udp,ports:all,address:0.0.0.0/0" \
      --output json | first_json_field id
  )
fi

droplet_ids=()
droplet_ips=()
for index in 0 1 2; do
  number=$((index + 1))
  name="mfenx-validator-$number"
  region="${REGIONS[$index]}"
  droplets_json=$(doctl_cmd compute droplet list --output json)
  droplet_id=$(printf '%s' "$droplets_json" | json_field_by_name id "$name")

  if [[ -z "$droplet_id" ]]; then
    echo "-> creating $name in $region"
    droplet_id=$(
      doctl_cmd compute droplet create "$name" \
        --region "$region" \
        --size "$SIZE" \
        --image "$IMAGE" \
        --ssh-keys "$SSH_KEY" \
        --tag-names "$TAG" \
        --project-id "$project_id" \
        --enable-monitoring \
        --enable-backups \
        --backup-policy-plan weekly \
        --backup-policy-weekday TUE \
        --backup-policy-hour 4 \
        --user-data-file "$ROOT/infra/digitalocean/cloud-init.yaml" \
        --wait \
        --output json | first_json_field id
    )
  else
    droplet_json=$(doctl_cmd compute droplet get "$droplet_id" --output json)
    python3 - "$name" "$region" "$SIZE" "$droplet_json" <<'PY'
import json
import sys

name, expected_region, expected_size, raw = sys.argv[1:]
data = json.loads(raw)
item = data[0] if isinstance(data, list) else data
region = item.get("region", {}).get("slug")
size = item.get("size_slug") or item.get("size", {}).get("slug")
if region != expected_region or size != expected_size:
    raise SystemExit(
        f"existing {name} does not match region={expected_region} size={expected_size}"
    )
PY
    echo "-> preserving existing $name"
  fi

  droplet_json=$(doctl_cmd compute droplet get "$droplet_id" --output json)
  public_ip=$(python3 - "$droplet_json" <<'PY'
import json
import sys

data = json.loads(sys.argv[1])
item = data[0] if isinstance(data, list) else data
for network in item.get("networks", {}).get("v4", []):
    if network.get("type") == "public":
        print(network["ip_address"])
        break
else:
    raise SystemExit("Droplet has no public IPv4 address")
PY
)
  droplet_ids+=("$droplet_id")
  droplet_ips+=("$public_ip")
done

droplet_ids_csv=$(IFS=,; echo "${droplet_ids[*]}")
load_balancers_json=$(doctl_cmd compute load-balancer list --output json)
load_balancer_id=$(
  printf '%s' "$load_balancers_json" |
    json_field_by_name id "$LOAD_BALANCER_NAME"
)
if [[ -z "$load_balancer_id" ]]; then
  load_balancer_id=$(
    printf '%s' "$load_balancers_json" |
      json_field_by_name id "$LEGACY_LOAD_BALANCER_NAME"
  )
fi
if [[ -z "$load_balancer_id" ]]; then
  echo "-> creating global load balancer"
  load_balancer_id=$(
    doctl_cmd compute load-balancer create \
      --name "$LOAD_BALANCER_NAME" \
      --type GLOBAL \
      --network EXTERNAL \
      --network-stack DUALSTACK \
      --droplet-ids "$droplet_ids_csv" \
      --glb-settings "target_protocol:http,target_port:80" \
      --health-check \
        "protocol:http,port:80,path:/healthz,check_interval_seconds:10,response_timeout_seconds:5,healthy_threshold:2,unhealthy_threshold:3" \
      --domains "name:$RPC_HOST is_managed:true" \
      --redirect-http-to-https \
      --tls-cipher-policy STRONG \
      --project-id "$project_id" \
      --wait \
      --output json | first_json_field id
  )
else
  load_balancer_json=$(
    doctl_cmd compute load-balancer get "$load_balancer_id" --output json
  )
  python3 - "$RPC_HOST" "$load_balancer_json" <<'PY'
import json
import sys

rpc_host, raw = sys.argv[1:]
data = json.loads(raw)
item = data[0] if isinstance(data, list) else data
domains = item.get("domains", [])
matches = [domain for domain in domains if domain.get("name") == rpc_host]
if len(matches) != 1 or matches[0].get("is_managed") is not True:
    raise SystemExit(
        "existing global load balancer does not use the required delegated-DNS "
        f"domain configuration for {rpc_host}; replace it before retrying"
    )
PY
fi

echo "-> reconciling validator firewall rules"
doctl_cmd compute firewall update "$firewall_id" \
  --name "$FIREWALL_NAME" \
  --tag-names "$TAG" \
  --inbound-rules \
    "protocol:tcp,ports:22,address:$SSH_CIDR protocol:tcp,ports:80,load_balancer_uid:$load_balancer_id protocol:tcp,ports:7001,tag:$TAG protocol:tcp,ports:7002,address:0.0.0.0/0 protocol:tcp,ports:9090,tag:$TAG protocol:tcp,ports:9100-9101,tag:$TAG" \
  --outbound-rules \
    "protocol:tcp,ports:all,address:0.0.0.0/0 protocol:udp,ports:all,address:0.0.0.0/0" \
  >/dev/null

load_balancer_json=$(
  doctl_cmd compute load-balancer get "$load_balancer_id" --output json
)
mkdir -p "$(dirname "$OUTPUT")"
python3 - \
  "$project_id" \
  "$firewall_id" \
  "$load_balancer_json" \
  "$REGIONS_CSV" \
  "$droplet_ids_csv" \
  "$(IFS=,; echo "${droplet_ips[*]}")" \
  "$RPC_HOST" \
  "$OUTPUT" <<'PY'
import json
import sys

(
    project_id,
    firewall_id,
    load_balancer_raw,
    regions_csv,
    droplet_ids_csv,
    droplet_ips_csv,
    rpc_host,
    output,
) = sys.argv[1:]
load_balancer_data = json.loads(load_balancer_raw)
load_balancer = (
    load_balancer_data[0]
    if isinstance(load_balancer_data, list)
    else load_balancer_data
)
manifest = {
    "droplets": [
        {"id": int(droplet_id), "public_ipv4": ip, "region": region}
        for droplet_id, ip, region in zip(
            droplet_ids_csv.split(","),
            droplet_ips_csv.split(","),
            regions_csv.split(","),
        )
    ],
    "firewall_id": firewall_id,
    "load_balancer": {
        "id": load_balancer.get("id"),
        "ip": load_balancer.get("ip"),
        "ipv6": load_balancer.get("ipv6"),
        "status": load_balancer.get("status"),
    },
    "project_id": project_id,
    "rpc_host": rpc_host,
}
with open(output, "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2, sort_keys=True)
    handle.write("\n")
print(json.dumps(manifest, indent=2, sort_keys=True))
PY

echo
echo "Infrastructure created. Next:"
echo "  1. Build the release binary."
echo "  2. Generate the sealed cluster bundle using the three public IPv4 addresses above."
echo "  3. Deploy with scripts/deploy_rpc_cluster.sh."
echo "  4. Delegate $DNS_ZONE from Namecheap to DigitalOcean's three nameservers."
echo "  5. Wait for managed DNS and TLS to become active before publication."
