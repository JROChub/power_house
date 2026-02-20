#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

fail() {
  echo "[FAIL] $*" >&2
  exit 1
}

check_contains() {
  local haystack="$1"
  local needle="$2"
  local label="$3"
  if [[ "$haystack" != *"$needle"* ]]; then
    fail "$label (missing: $needle)"
  fi
}

echo "[1/7] verify migration structs"
rg -q "pub struct MigrationProposal" src/net/governance.rs || fail "MigrationProposal missing"
rg -q "pub snapshot_height: u64" src/net/governance.rs || fail "MigrationProposal.snapshot_height missing"
rg -q "pub token_contract: String" src/net/governance.rs || fail "MigrationProposal.token_contract missing"
rg -q "pub conversion_ratio: u64" src/net/governance.rs || fail "MigrationProposal.conversion_ratio missing"
rg -q "pub treasury_mint: u64" src/net/governance.rs || fail "MigrationProposal.treasury_mint missing"

echo "[2/7] verify deterministic snapshot command"
rg -q "fn run_snapshot" src/commands/stake_snapshot.rs || fail "run_snapshot missing"
rg -q "stake snapshot" src/bin/julian.rs || fail "stake snapshot CLI wiring missing"

echo "[3/7] verify freeze hooks"
rg -q "PH_MIGRATION_MODE" src/net/migration.rs || fail "PH_MIGRATION_MODE hook missing"
rg -q "migration freeze active: stake bonding is disabled" src/bin/julian.rs || fail "stake freeze check missing"
rg -q "migration freeze active: blob ingestion disabled" src/net/swarm.rs || fail "blob freeze check missing"

echo "[4/7] verify help surfaces"
stake_help="$(cargo run --features net --bin julian --quiet -- stake --help 2>&1 || true)"
check_contains "$stake_help" "Usage: julian stake <show|fund|bond|snapshot|unbond|reward>" "stake help"

gov_help="$(cargo run --features net --bin julian --quiet -- governance --help 2>&1 || true)"
check_contains "$gov_help" "Usage: julian governance <propose-migration>" "governance help"

net_help="$(cargo run --features net --bin julian --quiet -- net --help 2>&1 || true)"
check_contains "$net_help" "Usage: julian net <start|anchor|verify-envelope>" "net help"

echo "[5/7] verify migration command help"
snap_help="$(cargo run --features net --bin julian --quiet -- stake snapshot --help 2>&1 || true)"
check_contains "$snap_help" "Usage: julian stake snapshot" "stake snapshot help"

proposal_help="$(cargo run --features net --bin julian --quiet -- governance propose-migration --help 2>&1 || true)"
check_contains "$proposal_help" "Usage: julian governance propose-migration" "governance propose-migration help"

echo "[6/7] verify net anchor compatibility"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
mkdir -p "$TMP_DIR/logs"

anchor_flagged="$(cargo run --features net --bin julian --quiet -- net anchor --log-dir "$TMP_DIR/logs" 2>&1 || true)"
check_contains "$anchor_flagged" "\"schema\"" "net anchor --log-dir output"

anchor_positional="$(cargo run --features net --bin julian --quiet -- net anchor "$TMP_DIR/logs" 2>&1 || true)"
check_contains "$anchor_positional" "\"schema\"" "net anchor positional output"

echo "[7/7] verify token mode flags exposed"
net_start_help="$(cargo run --features net --bin julian --quiet -- net start --help 2>&1 || true)"
check_contains "$net_start_help" "--token-mode" "net start token-mode flag"
check_contains "$net_start_help" "--token-oracle" "net start token-oracle flag"

echo "verify_migration_contract: PASS"
