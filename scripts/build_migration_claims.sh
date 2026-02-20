#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

if [[ $# -eq 0 ]] || [[ "${1:-}" == "-h" ]] || [[ "${1:-}" == "--help" ]]; then
  cat <<'EOF'
Usage: scripts/build_migration_claims.sh --snapshot <file> --output <file> [options]

Options are forwarded to:
  julian stake claims

Example:
  scripts/build_migration_claims.sh \
    --snapshot ./migration-snapshot.json \
    --output ./migration-claims.json \
    --mode native \
    --amount-source total \
    --conversion-ratio 1
EOF
  exit 0
fi

CARGO_BIN=${CARGO_BIN:-cargo}
MODE_SET=0
for arg in "$@"; do
  if [[ "$arg" == "--mode" ]]; then
    MODE_SET=1
    break
  fi
done
if [[ "$MODE_SET" == "1" ]]; then
  "$CARGO_BIN" run --features net --bin julian --quiet -- stake claims "$@"
else
  "$CARGO_BIN" run --features net --bin julian --quiet -- stake claims "$@" --mode native
fi
