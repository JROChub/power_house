#!/usr/bin/env bash
set -euo pipefail

if [[ ${1:-} == "" ]]; then
  echo "Usage: $0 <julian-binary-path>" >&2
  exit 1
fi

BIN_SRC="$1"
SERVICE=${PH_SERVICE_NAME:-powerhouse-boot}
RELEASE_ROOT=${PH_RELEASE_ROOT:-/opt/jrocnet/releases}

if [[ ! -f "$BIN_SRC" ]]; then
  echo "Binary not found: $BIN_SRC" >&2
  exit 1
fi

VERSION=$({ $BIN_SRC --version 2>/dev/null || true; } | awk '{print $NF}')
if [[ -z "$VERSION" ]]; then
  VERSION=$(date -u +%Y%m%dT%H%M%SZ)
fi

DEST_DIR="$RELEASE_ROOT/$VERSION"
mkdir -p "$DEST_DIR"
install -m 0755 "$BIN_SRC" "$DEST_DIR/julian"

if [[ -L "$RELEASE_ROOT/current" ]]; then
  ln -sfn "$(readlink -f "$RELEASE_ROOT/current")" "$RELEASE_ROOT/previous"
fi
ln -sfn "$DEST_DIR" "$RELEASE_ROOT/current"
ln -sfn "$RELEASE_ROOT/current/julian" /usr/local/bin/julian

systemctl restart "$SERVICE"

printf 'deployed %s to %s\n' "$VERSION" "$DEST_DIR"
