#!/usr/bin/env bash
set -euo pipefail

if [[ ${1:-} == "" ]]; then
  echo "Usage: $0 <backup-archive>" >&2
  exit 1
fi

ARCHIVE="$1"
SERVICE=${PH_SERVICE_NAME:-powerhouse-boot}
RESTORE_DIR=${PH_RESTORE_DIR:-/}

if [[ ! -f "$ARCHIVE" ]]; then
  echo "Archive not found: $ARCHIVE" >&2
  exit 1
fi

systemctl stop "$SERVICE" || true

case "$ARCHIVE" in
  *.tar.zst)
    zstd -dc "$ARCHIVE" | tar -xf - -C "$RESTORE_DIR"
    ;;
  *.tar.gz)
    tar -xzf "$ARCHIVE" -C "$RESTORE_DIR"
    ;;
  *)
    echo "Unsupported archive format" >&2
    exit 1
    ;;
 esac

systemctl start "$SERVICE"

echo "restore complete: $ARCHIVE"
