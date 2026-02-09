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

copy_ops() {
  local host="$1"
  echo "-> Deploying ops scripts to $host"
  ssh $SSH_OPTS "$host" 'install -d /usr/local/lib/powerhouse'
  ssh $SSH_OPTS "$host" 'install -d /var/backups/powerhouse /var/log/powerhouse /var/lib/powerhouse/ops'
  scp $SSH_OPTS "$ROOT/infra/ops/alert.sh" "$host:/usr/local/lib/powerhouse/alert.sh"
  scp $SSH_OPTS "$ROOT/infra/ops/healthcheck.py" "$host:/usr/local/lib/powerhouse/healthcheck.py"
  scp $SSH_OPTS "$ROOT/infra/ops/backup.sh" "$host:/usr/local/lib/powerhouse/backup.sh"
  scp $SSH_OPTS "$ROOT/infra/ops/restore.sh" "$host:/usr/local/lib/powerhouse/restore.sh"
  scp $SSH_OPTS "$ROOT/infra/ops/deploy_release.sh" "$host:/usr/local/lib/powerhouse/deploy_release.sh"
  scp $SSH_OPTS "$ROOT/infra/ops/rollback.sh" "$host:/usr/local/lib/powerhouse/rollback.sh"
  scp $SSH_OPTS "$ROOT/infra/ops/journal_export.sh" "$host:/usr/local/lib/powerhouse/journal_export.sh"
  scp $SSH_OPTS "$ROOT/infra/ops/metrics_snapshot.sh" "$host:/usr/local/lib/powerhouse/metrics_snapshot.sh"
  ssh $SSH_OPTS "$host" 'chmod +x /usr/local/lib/powerhouse/*.sh /usr/local/lib/powerhouse/*.py'
  scp $SSH_OPTS "$ROOT/infra/scripts/powerhouse-boot.sh" "$host:/usr/local/bin/powerhouse-boot.sh"
  ssh $SSH_OPTS "$host" 'chmod +x /usr/local/bin/powerhouse-boot.sh'
}

copy_timers() {
  local host="$1"
  echo "-> Deploying timers to $host"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-healthcheck@.service" "$host:/etc/systemd/system/powerhouse-healthcheck@.service"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-healthcheck@.timer" "$host:/etc/systemd/system/powerhouse-healthcheck@.timer"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-backup@.service" "$host:/etc/systemd/system/powerhouse-backup@.service"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-backup@.timer" "$host:/etc/systemd/system/powerhouse-backup@.timer"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-log-export@.service" "$host:/etc/systemd/system/powerhouse-log-export@.service"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-log-export@.timer" "$host:/etc/systemd/system/powerhouse-log-export@.timer"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-metrics@.service" "$host:/etc/systemd/system/powerhouse-metrics@.service"
  scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-metrics@.timer" "$host:/etc/systemd/system/powerhouse-metrics@.timer"
}

echo "-> Copying unit to $BOOT1"
scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-boot1.service" "$BOOT1:/etc/systemd/system/powerhouse-boot1.service"
copy_ops "$BOOT1"
copy_timers "$BOOT1"
echo "-> Reloading and restarting on $BOOT1"
ssh $SSH_OPTS "$BOOT1" 'systemctl daemon-reload; systemctl reset-failed powerhouse-boot1.service || true; systemctl enable --now powerhouse-boot1.service; systemctl enable --now powerhouse-healthcheck@boot1.timer powerhouse-backup@boot1.timer powerhouse-log-export@boot1.timer powerhouse-metrics@boot1.timer; systemd-analyze verify /etc/systemd/system/powerhouse-boot1.service || true; systemctl status --no-pager -l powerhouse-boot1.service | sed -n "1,40p"'

echo "-> Copying unit to $BOOT2"
scp $SSH_OPTS "$ROOT/infra/systemd/powerhouse-boot2.service" "$BOOT2:/etc/systemd/system/powerhouse-boot2.service"
copy_ops "$BOOT2"
copy_timers "$BOOT2"
echo "-> Reloading and restarting on $BOOT2"
ssh $SSH_OPTS "$BOOT2" 'systemctl daemon-reload; systemctl reset-failed powerhouse-boot2.service || true; systemctl enable --now powerhouse-boot2.service; systemctl enable --now powerhouse-healthcheck@boot2.timer powerhouse-backup@boot2.timer powerhouse-log-export@boot2.timer powerhouse-metrics@boot2.timer; systemd-analyze verify /etc/systemd/system/powerhouse-boot2.service || true; systemctl status --no-pager -l powerhouse-boot2.service | sed -n "1,40p"'

echo "-> Quick metrics check"
ssh $SSH_OPTS "$BOOT1" 'curl -s 127.0.0.1:9100 | egrep "anchors_(received|verified)_total|finality_events_total|gossipsub_rejects_total" || true'
ssh $SSH_OPTS "$BOOT2" 'curl -s 127.0.0.1:9100 | egrep "anchors_(received|verified)_total|finality_events_total|gossipsub_rejects_total" || true'

echo "Done."
