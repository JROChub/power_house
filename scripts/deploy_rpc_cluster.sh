#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage:
  scripts/deploy_rpc_cluster.sh <bundle-dir> <validator-1-ssh> <validator-2-ssh> <validator-3-ssh>

Environment:
  JULIAN_BIN  Release binary to deploy (default: target/release/julian)
  SSH_OPTS    Additional ssh/scp options
EOF
  exit 1
}

[[ $# -eq 4 ]] || usage

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
BUNDLE="$(realpath "$1")"
shift
HOSTS=("$@")
JULIAN_BIN="${JULIAN_BIN:-$ROOT/target/release/julian}"
SSH_ARGS=()
if [[ -n "${SSH_OPTS:-}" ]]; then
  read -r -a SSH_ARGS <<<"$SSH_OPTS"
fi

required_bundle_files=(
  cluster-manifest.json
  native-validators.json
  powerhouse-common.env
  stake_registry.json
  validator-registry.json
)

[[ -d "$BUNDLE" ]] || {
  echo "bundle directory not found: $BUNDLE" >&2
  exit 1
}
[[ -x "$JULIAN_BIN" ]] || {
  echo "release binary not found or not executable: $JULIAN_BIN" >&2
  echo "run: cargo build --release --locked --features net --bin julian" >&2
  exit 1
}
EXPECTED_VERSION=$("$JULIAN_BIN" --version | awk '{print $2}')
for file in "${required_bundle_files[@]}"; do
  [[ -f "$BUNDLE/$file" ]] || {
    echo "missing bundle file: $BUNDLE/$file" >&2
    exit 1
  }
done

manifest_valid=$(
  python3 - "$BUNDLE/cluster-manifest.json" <<'PY'
import json
import sys

manifest = json.load(open(sys.argv[1], encoding="utf-8"))
valid = (
    manifest.get("chain_id") == 177155
    and manifest.get("quorum") == 2
    and len(manifest.get("validators", [])) == 3
)
print("yes" if valid else "no")
PY
)
[[ "$manifest_valid" == "yes" ]] || {
  echo "bundle manifest is not a chain 177155 three-validator quorum-2 bundle" >&2
  exit 1
}

copy_ops() {
  local host=$1
  ssh "${SSH_ARGS[@]}" "$host" \
    "install -d -m 0750 /etc/powerhouse /var/backups/powerhouse /var/log/powerhouse /usr/local/lib/powerhouse"
  for script in alert.sh backup.sh healthcheck.py journal_export.sh metrics_snapshot.sh restore.sh; do
    scp "${SSH_ARGS[@]}" "$ROOT/infra/ops/$script" "$host:/usr/local/lib/powerhouse/$script"
  done
  ssh "${SSH_ARGS[@]}" "$host" "chmod 0755 /usr/local/lib/powerhouse/*"
}

copy_units() {
  local host=$1
  scp "${SSH_ARGS[@]}" "$ROOT/infra/systemd/powerhouse-node@.service" \
    "$host:/etc/systemd/system/powerhouse-node@.service"
  for unit in \
    powerhouse-healthcheck@.service powerhouse-healthcheck@.timer \
    powerhouse-backup@.service powerhouse-backup@.timer \
    powerhouse-log-export@.service powerhouse-log-export@.timer \
    powerhouse-metrics@.service powerhouse-metrics@.timer; do
    scp "${SSH_ARGS[@]}" "$ROOT/infra/systemd/$unit" "$host:/etc/systemd/system/$unit"
  done
  scp "${SSH_ARGS[@]}" "$ROOT/infra/scripts/powerhouse-boot.sh" \
    "$host:/usr/local/bin/powerhouse-boot.sh"
  ssh "${SSH_ARGS[@]}" "$host" "chmod 0755 /usr/local/bin/powerhouse-boot.sh"
}

deploy_node() {
  local index=$1
  local host=$2
  local node="validator-$index"
  local env_file="$BUNDLE/powerhouse-$node.env"
  local key_file="$BUNDLE/$node.key"

  [[ -f "$env_file" ]] || {
    echo "missing node environment: $env_file" >&2
    exit 1
  }
  [[ -f "$key_file" ]] || {
    echo "missing node key: $key_file" >&2
    exit 1
  }

  echo "-> staging $node on $host"
  copy_ops "$host"
  copy_units "$host"

  scp "${SSH_ARGS[@]}" "$JULIAN_BIN" "$host:/tmp/julian-powerhouse.new"
  scp "${SSH_ARGS[@]}" "$BUNDLE/powerhouse-common.env" \
    "$host:/etc/powerhouse/.powerhouse-common.env.upload"
  scp "${SSH_ARGS[@]}" "$env_file" \
    "$host:/etc/powerhouse/.powerhouse-$node.env.upload"
  scp "${SSH_ARGS[@]}" "$key_file" \
    "$host:/etc/powerhouse/.$node.key.upload"
  scp "${SSH_ARGS[@]}" "$BUNDLE/native-validators.json" \
    "$host:/etc/powerhouse/.native-validators.json.upload"
  scp "${SSH_ARGS[@]}" "$BUNDLE/stake_registry.json" \
    "$host:/etc/powerhouse/.stake_registry.json.upload"
  scp "${SSH_ARGS[@]}" "$BUNDLE/validator-registry.json" \
    "$host:/etc/powerhouse/.validator-registry.json.upload"

  ssh "${SSH_ARGS[@]}" "$host" env "NODE=$node" bash -s <<'REMOTE'
set -euo pipefail
state="/var/lib/powerhouse/$NODE"
install -d -m 0750 "$state/logs"
install -m 0640 /etc/powerhouse/.powerhouse-common.env.upload /etc/powerhouse/powerhouse-common.env
install -m 0640 "/etc/powerhouse/.powerhouse-$NODE.env.upload" "/etc/powerhouse/powerhouse-$NODE.env"
install -m 0600 "/etc/powerhouse/.$NODE.key.upload" "/etc/powerhouse/$NODE.key"
install -m 0640 /etc/powerhouse/.native-validators.json.upload /etc/powerhouse/native-validators.json
install -m 0640 /etc/powerhouse/.validator-registry.json.upload /etc/powerhouse/validator-registry.json

if [[ -e "$state/native_chain_state.json" ]]; then
  echo "preserving finalized native chain state for $NODE"
elif [[ -e "$state/stake_registry.json" ]]; then
  cmp -s /etc/powerhouse/.stake_registry.json.upload "$state/stake_registry.json" || {
    echo "existing genesis registry differs for $NODE" >&2
    exit 1
  }
else
  install -m 0640 /etc/powerhouse/.stake_registry.json.upload "$state/stake_registry.json"
fi

rm -f \
  /etc/powerhouse/.powerhouse-common.env.upload \
  "/etc/powerhouse/.powerhouse-$NODE.env.upload" \
  "/etc/powerhouse/.$NODE.key.upload" \
  /etc/powerhouse/.native-validators.json.upload \
  /etc/powerhouse/.stake_registry.json.upload \
  /etc/powerhouse/.validator-registry.json.upload

systemctl daemon-reload
systemd-analyze verify "/etc/systemd/system/powerhouse-node@.service"
REMOTE
}

for index in 1 2 3; do
  deploy_node "$index" "${HOSTS[$((index - 1))]}"
done

for index in 1 2 3; do
  node="validator-$index"
  host="${HOSTS[$((index - 1))]}"
  echo "-> rolling $node on $host to $EXPECTED_VERSION"
  ssh "${SSH_ARGS[@]}" "$host" env \
    "NODE=$node" "EXPECTED_VERSION=$EXPECTED_VERSION" bash -s <<'REMOTE'
set -euo pipefail
service="powerhouse-node@$NODE.service"
release_root=/opt/powerhouse/releases
install -d -m 0750 "$release_root"
backup="$release_root/julian-before-$EXPECTED_VERSION-$(date -u +%Y%m%dT%H%M%SZ)"

if [[ -x /usr/local/bin/julian ]]; then
  cp -p /usr/local/bin/julian "$backup"
fi

rollback() {
  echo "rolling back $NODE" >&2
  if [[ -x "$backup" ]]; then
    install -m 0755 "$backup" /usr/local/bin/julian
    systemctl restart "$service" || true
  fi
}
trap rollback ERR

install -m 0755 /tmp/julian-powerhouse.new /usr/local/bin/julian
systemctl daemon-reload
systemctl enable \
  "$service" \
  "powerhouse-healthcheck@$NODE.timer" \
  "powerhouse-backup@$NODE.timer" \
  "powerhouse-log-export@$NODE.timer" \
  "powerhouse-metrics@$NODE.timer" >/dev/null
systemctl restart "$service"
systemctl start \
  "powerhouse-healthcheck@$NODE.timer" \
  "powerhouse-backup@$NODE.timer" \
  "powerhouse-log-export@$NODE.timer" \
  "powerhouse-metrics@$NODE.timer"

for _ in $(seq 1 30); do
  if systemctl is-active --quiet "$service" \
    && curl --fail --silent http://127.0.0.1:8545/healthz >/dev/null; then
    break
  fi
  sleep 2
done

systemctl is-active --quiet "$service"
curl --fail --silent --show-error http://127.0.0.1:8545/healthz >/dev/null
actual=$(/usr/local/bin/julian --version | awk '{print $2}')
[[ "$actual" == "$EXPECTED_VERSION" ]]
client=$(
  curl --fail --silent --show-error \
    -H 'content-type: application/json' \
    --data '{"jsonrpc":"2.0","id":1,"method":"web3_clientVersion","params":[]}' \
    http://127.0.0.1:8545
)
grep -q "power-house/$EXPECTED_VERSION/" <<<"$client"
rm -f /tmp/julian-powerhouse.new
trap - ERR
echo "$NODE healthy on Power House $EXPECTED_VERSION"
REMOTE
done

echo "RPC cluster deployment completed at Power House $EXPECTED_VERSION."
