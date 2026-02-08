#!/usr/bin/env bash
set -euo pipefail

SERVICE=${PH_SERVICE_NAME:-powerhouse-boot}
EXPORT_DIR=${PH_LOG_EXPORT_DIR:-/var/log/powerhouse}
RETENTION_DAYS=${PH_LOG_EXPORT_RETENTION_DAYS:-7}
SHIP_HOST=${PH_LOG_SHIP_HOST:-}
SHIP_USER=${PH_LOG_SHIP_USER:-root}
SHIP_PATH=${PH_LOG_SHIP_PATH:-/var/log/powerhouse/remote}
SHIP_PORT=${PH_LOG_SHIP_PORT:-22}
SHIP_KEY=${PH_LOG_SHIP_KEY:-}

export HOME=/root

mkdir -p "$EXPORT_DIR"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT="$EXPORT_DIR/${SERVICE}-${STAMP}.log"

journalctl -u "$SERVICE" --since "1 hour ago" --no-pager > "$OUT"

gzip -f "$OUT"

find "$EXPORT_DIR" -type f -name "${SERVICE}-*.log.gz" -mtime "+$RETENTION_DAYS" -delete || true

if [[ -n "$SHIP_HOST" ]]; then
  DEST="${SHIP_USER}@${SHIP_HOST}:${SHIP_PATH}"
  SSH_OPTS="-o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/root/.ssh/known_hosts -o ConnectTimeout=5 -p ${SHIP_PORT}"
  if [[ -n "$SHIP_KEY" ]]; then
    SSH_OPTS="$SSH_OPTS -i ${SHIP_KEY}"
  fi
  logger -t powerhouse-log-ship "ship start dest=${DEST} key=${SHIP_KEY:-none}"
  REMOTE_FILE="$(basename "${OUT}.gz")"
  if ssh $SSH_OPTS "$SHIP_USER@$SHIP_HOST" "mkdir -p '$SHIP_PATH'"; then
    if cat "${OUT}.gz" | ssh $SSH_OPTS "$SHIP_USER@$SHIP_HOST" "cat > '$SHIP_PATH/$REMOTE_FILE'"; then
      logger -t powerhouse-log-ship "shipped ${OUT}.gz to ${DEST}"
    else
      logger -t powerhouse-log-ship "ship failed for ${OUT}.gz to ${DEST}: stream error"
    fi
  else
    logger -t powerhouse-log-ship "ship failed for ${OUT}.gz to ${DEST}: ssh unavailable"
  fi
fi

echo "exported logs to ${OUT}.gz"
