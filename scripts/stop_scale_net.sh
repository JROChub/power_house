#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
PIDS_FILE="${PIDS_FILE:-$ROOT_DIR/logs/scale_pids.txt}"

if [[ ! -f "$PIDS_FILE" ]]; then
  echo "no pids file found at $PIDS_FILE"
  exit 0
fi

while read -r pid; do
  if [[ -n "$pid" ]]; then
    kill "$pid" 2>/dev/null || true
  fi
done < "$PIDS_FILE"

echo "stopped nodes listed in $PIDS_FILE"
