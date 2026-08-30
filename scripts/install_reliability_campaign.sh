#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage:
  scripts/install_reliability_campaign.sh \
    --release <version> \
    --node-1 <user@host> \
    --node-2 <user@host> \
    --node-3 <user@host> \
    [--duration-seconds 259200] [--force]

Installs and starts the external 72-hour reliability controller as a hardened
user service. Existing campaign state is preserved unless --force is supplied.
EOF
  exit 1
}

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
RELEASE=""
NODE_1=""
NODE_2=""
NODE_3=""
DURATION=259200
FORCE=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) RELEASE="${2:-}"; shift 2 ;;
    --node-1) NODE_1="${2:-}"; shift 2 ;;
    --node-2) NODE_2="${2:-}"; shift 2 ;;
    --node-3) NODE_3="${2:-}"; shift 2 ;;
    --duration-seconds) DURATION="${2:-}"; shift 2 ;;
    --force) FORCE=true; shift ;;
    -h|--help) usage ;;
    *) echo "unknown argument: $1" >&2; usage ;;
  esac
done

[[ "$RELEASE" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || usage
[[ -n "$NODE_1" && -n "$NODE_2" && -n "$NODE_3" ]] || usage
[[ "$DURATION" =~ ^[0-9]+$ && "$DURATION" -ge 60 ]] || usage

STATE_DIR="$HOME/.local/state/powerhouse/reliability"
CONFIG_DIR="$HOME/.config/powerhouse"
LIB_DIR="$HOME/.local/lib/powerhouse"
UNIT_DIR="$HOME/.config/systemd/user"
CONFIG="$CONFIG_DIR/reliability-campaign.json"

if systemctl --user is-active --quiet powerhouse-reliability-campaign.service; then
  if [[ "$FORCE" != true ]]; then
    echo "a reliability campaign is already running" >&2
    exit 1
  fi
  systemctl --user stop powerhouse-reliability-campaign.service
fi

if [[ -f "$STATE_DIR/campaign-state.json" ]]; then
  if [[ "$FORCE" != true ]]; then
    echo "existing campaign state found at $STATE_DIR; use --force to archive it" >&2
    exit 1
  fi
  archive="$HOME/.local/state/powerhouse/reliability-$(date -u +%Y%m%dT%H%M%SZ)"
  mv "$STATE_DIR" "$archive"
  echo "archived previous campaign at $archive"
fi

for target in "$NODE_1" "$NODE_2" "$NODE_3"; do
  ssh -F /dev/null -o BatchMode=yes -o StrictHostKeyChecking=accept-new \
    "$target" "/usr/local/bin/julian --version && curl -fsS http://127.0.0.1:8545/healthz >/dev/null"
done

install -d -m 0700 "$STATE_DIR" "$CONFIG_DIR"
install -d -m 0755 "$LIB_DIR" "$UNIT_DIR"
install -m 0755 "$ROOT/infra/monitoring/reliability_campaign.py" \
  "$LIB_DIR/reliability_campaign.py"
install -m 0644 "$ROOT/infra/systemd/powerhouse-reliability-campaign.service" \
  "$UNIT_DIR/powerhouse-reliability-campaign.service"

python3 - "$CONFIG" "$STATE_DIR" "$RELEASE" "$DURATION" "$NODE_1" "$NODE_2" "$NODE_3" <<'PY'
import json
import os
from pathlib import Path
import sys

path, state_dir, release, duration, node1, node2, node3 = sys.argv[1:]
targets = [node1, node2, node3]
nodes = []
for index, target in enumerate(targets, start=1):
    nodes.append(
        {
            "name": f"validator-{index}",
            "target": target,
            "service": f"powerhouse-node@validator-{index}.service",
            "state_path": f"/var/lib/powerhouse/validator-{index}/native_chain_state.json",
            "observer_registry_path": (
                "/var/lib/powerhouse/observer-intake/observer-registry.json"
                if index == 1
                else "/var/lib/powerhouse/monitoring/observer-registry.json"
            ),
        }
    )

config = {
    "schema": "power-house-reliability-config-v1",
    "state_dir": state_dir,
    "duration_seconds": int(duration),
    "sample_interval_seconds": 60,
    "burst_interval_seconds": 3600,
    "burst_requests": 30,
    "recovery_timeout_seconds": 90,
    "probe_attempts": 3,
    "probe_retry_delay_seconds": 0.75,
    "http_timeout_seconds": 6,
    "ssh_timeout_seconds": 12,
    "max_parallel_probes": 8,
    "max_rpc_p95_ms": 1000,
    "expected_chain_id": 177155,
    "expected_release": release,
    "rpc_url": "https://rpc.mfenx.com",
    "status_url": "https://rpc.mfenx.com/network-status.json",
    "intake_url": "https://rpc.mfenx.com/observer-intake-healthz",
    "primary_node": "validator-1",
    "nodes": nodes,
    "ssh_options": [
        "-F", "/dev/null",
        "-o", "BatchMode=yes",
        "-o", "ConnectTimeout=8",
        "-o", "StrictHostKeyChecking=accept-new",
    ],
    "publish_targets": targets,
    "publish_path": "/var/lib/powerhouse/reliability/campaign-status.json",
    "drills": [
        {"id": "validator-failover-6h", "kind": "validator_failover", "offset_seconds": 21600},
        {"id": "intake-recovery-24h", "kind": "intake_recovery", "offset_seconds": 86400},
        {"id": "replica-recovery-48h", "kind": "replica_recovery", "offset_seconds": 172800},
        {"id": "validator-failover-66h", "kind": "validator_failover", "offset_seconds": 237600},
    ],
}

# Short test campaigns keep the same ordering while fitting every drill inside
# the requested duration.
if int(duration) < 237601:
    fractions = (0.08, 0.33, 0.66, 0.90)
    for drill, fraction in zip(config["drills"], fractions):
        drill["offset_seconds"] = max(1, int(int(duration) * fraction))

temporary = Path(f"{path}.tmp")
temporary.write_text(json.dumps(config, indent=2, sort_keys=True) + "\n", encoding="utf-8")
os.chmod(temporary, 0o600)
os.replace(temporary, path)
PY

python3 -m py_compile "$LIB_DIR/reliability_campaign.py"
python3 "$LIB_DIR/reliability_campaign.py" preflight \
  --config "$CONFIG" \
  --samples 10 \
  --interval-seconds 2
systemctl --user daemon-reload
systemctl --user enable --now powerhouse-reliability-campaign.service

linger=$(loginctl show-user "$USER" -p Linger --value 2>/dev/null || true)
if [[ "$linger" != "yes" ]]; then
  if command -v sudo >/dev/null 2>&1 && sudo -n loginctl enable-linger "$USER"; then
    linger=yes
  fi
fi
[[ "$linger" == "yes" ]] || {
  echo "campaign started, but user lingering is not enabled" >&2
  echo "run: sudo loginctl enable-linger $USER" >&2
  exit 2
}

for _ in $(seq 1 30); do
  [[ -f "$STATE_DIR/campaign-status.json" ]] && break
  sleep 1
done
systemctl --user is-active --quiet powerhouse-reliability-campaign.service
[[ -f "$STATE_DIR/campaign-status.json" ]]
python3 "$LIB_DIR/reliability_campaign.py" status --config "$CONFIG"
