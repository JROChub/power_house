#!/usr/bin/env bash
set -euo pipefail

SERVICE=${PH_SERVICE_NAME:-powerhouse-boot}
RELEASE_ROOT=${PH_RELEASE_ROOT:-/opt/powerhouse/releases}

if [[ ! -L "$RELEASE_ROOT/previous" ]]; then
  echo "No previous release found" >&2
  exit 1
fi

PREV=$(readlink -f "$RELEASE_ROOT/previous")
CUR=$(readlink -f "$RELEASE_ROOT/current" || true)

if [[ -z "$PREV" || ! -d "$PREV" ]]; then
  echo "Invalid previous release" >&2
  exit 1
fi

ln -sfn "$PREV" "$RELEASE_ROOT/current"
ln -sfn "$RELEASE_ROOT/current/julian" /usr/local/bin/julian

if [[ -n "$CUR" && -d "$CUR" ]]; then
  ln -sfn "$CUR" "$RELEASE_ROOT/previous"
fi

systemctl restart "$SERVICE"

echo "rollback complete: $(basename "$PREV")"
