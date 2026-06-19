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
expect_output "navigate" rootprint --help
expect_output "requires no network access" identity --help
expect_output "never changes the Power House core fingerprint" attach-external-proof --help
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
expect_output "libp2p peer ID" key-info --help
expect_output "Diagnose, set up, register" observer --help
expect_output "status   Read live admission status" observer --help
expect_output "--intake-url" observer --help
expect_output "default public observer bootnodes" observer --help
expect_output "signed by the validator identity" validator-registry --help
expect_output "register --node-id" validator-registry --help
expect_output "register --node-id" observer-registry --help

KEY_INFO=$("$JULIAN" key-info ed25519://cli-test-validator --json)
python3 -c '
import json
import sys

info = json.loads(sys.argv[1])
assert len(info["public_key_b64"]) == 44
assert info["peer_id"].startswith("12D3KooW")
' "$KEY_INFO"

if "$JULIAN" definitely-not-a-command >/dev/null 2>&1; then
  echo "unknown command unexpectedly succeeded" >&2
  exit 1
fi

printf 'test_cli: PASS\n'
