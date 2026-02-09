#!/usr/bin/env bash
set -euo pipefail

NODE_ID=${PH_NODE_ID:-node}
BACKUP_DIR=${PH_BACKUP_DIR:-/var/backups/powerhouse}
SOURCES=${PH_BACKUP_SOURCES:-}
RETENTION_DAYS=${PH_BACKUP_RETENTION_DAYS:-14}

if [[ -z "$SOURCES" ]]; then
  echo "PH_BACKUP_SOURCES is empty" >&2
  exit 1
fi

mkdir -p "$BACKUP_DIR"

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
ARCHIVE_BASE="powerhouse-${NODE_ID}-${STAMP}"

if command -v zstd >/dev/null 2>&1; then
  ARCHIVE="$BACKUP_DIR/${ARCHIVE_BASE}.tar.zst"
  tar --ignore-failed-read --warning=no-file-changed -cf - $SOURCES | zstd -T0 -19 -o "$ARCHIVE"
else
  ARCHIVE="$BACKUP_DIR/${ARCHIVE_BASE}.tar.gz"
  tar --ignore-failed-read --warning=no-file-changed -czf "$ARCHIVE" $SOURCES
fi

find "$BACKUP_DIR" -type f -name "powerhouse-${NODE_ID}-*.tar.*" -mtime "+$RETENTION_DAYS" -delete || true

echo "backup: $ARCHIVE"
