#!/usr/bin/env bash
set -euo pipefail

# Deploy known-good systemd unit files for boot1 and boot2, then reload and restart.
# Usage:
#   scripts/deploy_units.sh root@137.184.33.2 root@146.190.126.101
# Optional env:
#   SSH_OPTS="-o StrictHostKeyChecking=accept-new" ./scripts/deploy_units.sh root@boot1 root@boot2

if [[ ${1:-} == "" || ${2:-} == "" ]]; then
  echo "Usage: $0 <boot1_host> <boot2_host>" >&2
  exit 1
fi

BOOT1="$1"
BOOT2="$2"
SSH_OPTS=${SSH_OPTS:-}

here() { 
  # repo root relative to this script
  local d
  d=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
  printf '%s' "$d"
}

ROOT="$(here)"

echo "-> Copying unit to $BOOT1"
scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-boot1.service" "$BOOT1:/etc/systemd/system/powerhouse-boot1.service"
echo "-> Reloading and restarting on $BOOT1"
ssh $SSH_OPTS "$BOOT1" 'systemctl daemon-reload; systemctl reset-failed powerhouse-boot1.service || true; systemctl enable --now powerhouse-boot1.service; systemd-analyze verify /etc/systemd/system/powerhouse-boot1.service || true; systemctl status --no-pager -l powerhouse-boot1.service | sed -n "1,40p"'

echo "-> Copying unit to $BOOT2"
scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-boot2.service" "$BOOT2:/etc/systemd/system/powerhouse-boot2.service"
echo "-> Reloading and restarting on $BOOT2"
ssh $SSH_OPTS "$BOOT2" 'systemctl daemon-reload; systemctl reset-failed powerhouse-boot2.service || true; systemctl enable --now powerhouse-boot2.service; systemd-analyze verify /etc/systemd/system/powerhouse-boot2.service || true; systemctl status --no-pager -l powerhouse-boot2.service | sed -n "1,40p"'

echo "-> Quick metrics check"
ssh $SSH_OPTS "$BOOT1" 'curl -s 127.0.0.1:9100 | egrep "anchors_(received|verified)_total|finality_events_total|gossipsub_rejects_total" || true'
ssh $SSH_OPTS "$BOOT2" 'curl -s 127.0.0.1:9100 | egrep "anchors_(received|verified)_total|finality_events_total|gossipsub_rejects_total" || true'

echo "Done."
