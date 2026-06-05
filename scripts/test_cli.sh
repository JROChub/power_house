#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

CARGO_BIN="${CARGO_BIN:-cargo}"
"$CARGO_BIN" build --locked --features net --bin julian
JULIAN="$ROOT_DIR/target/debug/julian"
VERSION=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')

expect_output() {
  local expected=$1
  shift
  local output
  output=$("$JULIAN" "$@")
  if [[ "$output" != *"$expected"* ]]; then
    echo "missing '$expected' in: julian $*" >&2
    echo "$output" >&2
    exit 1
  fi
}

expect_output "Power-House JULIAN" --help
expect_output "julian $VERSION" --version
expect_output "verify-proof" node --help
expect_output "streaming sum-check" scale_sumcheck --help
expect_output "verify-envelope" net --help
expect_output "verify-envelope" network --help
expect_output "attestation-quorum" net start --help
expect_output "apply-claims" stake --help
expect_output "propose-migration" governance --help
expect_output "execute-burn-intents" migration --help
expect_output "settle-file" rollup --help
expect_output "encrypted Ed25519 identity" keygen --help

if "$JULIAN" definitely-not-a-command >/dev/null 2>&1; then
  echo "unknown command unexpectedly succeeded" >&2
  exit 1
fi

printf 'test_cli: PASS\n'
