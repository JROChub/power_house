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

echo "[1/10] verify migration structs"
rg -q "pub struct MigrationProposal" src/net/governance.rs || fail "MigrationProposal missing"
rg -q "pub snapshot_height: u64" src/net/governance.rs || fail "MigrationProposal.snapshot_height missing"
rg -q "pub token_contract: String" src/net/governance.rs || fail "MigrationProposal.token_contract missing"
rg -q "pub conversion_ratio: u64" src/net/governance.rs || fail "MigrationProposal.conversion_ratio missing"
rg -q "pub treasury_mint: u64" src/net/governance.rs || fail "MigrationProposal.treasury_mint missing"

echo "[2/10] verify deterministic snapshot + claims commands"
rg -q "fn run_snapshot" src/commands/stake_snapshot.rs || fail "run_snapshot missing"
rg -q "fn run_build_claims" src/commands/migration_claims.rs || fail "run_build_claims missing"
rg -q "stake snapshot" src/bin/julian.rs || fail "stake snapshot CLI wiring missing"
rg -q "stake claims" src/bin/julian.rs || fail "stake claims CLI wiring missing"

echo "[3/10] verify freeze hooks"
rg -q "PH_MIGRATION_MODE" src/net/migration.rs || fail "PH_MIGRATION_MODE hook missing"
rg -q "migration freeze active: stake bonding is disabled" src/bin/julian.rs || fail "stake freeze check missing"
rg -q "migration freeze active: blob ingestion disabled" src/net/swarm.rs || fail "blob freeze check missing"

echo "[4/10] verify help surfaces"
stake_help="$(cargo run --features net --bin julian --quiet -- stake --help 2>&1 || true)"
check_contains "$stake_help" "Usage: julian stake <show|fund|bond|snapshot|claims|unbond|reward>" "stake help"

gov_help="$(cargo run --features net --bin julian --quiet -- governance --help 2>&1 || true)"
check_contains "$gov_help" "Usage: julian governance <propose-migration>" "governance help"

net_help="$(cargo run --features net --bin julian --quiet -- net --help 2>&1 || true)"
check_contains "$net_help" "Usage: julian net <start|anchor|verify-envelope>" "net help"

echo "[5/10] verify migration command help"
snap_help="$(cargo run --features net --bin julian --quiet -- stake snapshot --help 2>&1 || true)"
check_contains "$snap_help" "Usage: julian stake snapshot" "stake snapshot help"

claims_help="$(cargo run --features net --bin julian --quiet -- stake claims --help 2>&1 || true)"
check_contains "$claims_help" "Usage: julian stake claims" "stake claims help"

proposal_help="$(cargo run --features net --bin julian --quiet -- governance propose-migration --help 2>&1 || true)"
check_contains "$proposal_help" "Usage: julian governance propose-migration" "governance propose-migration help"

echo "[6/10] verify net anchor compatibility"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
mkdir -p "$TMP_DIR/logs"

anchor_flagged="$(cargo run --features net --bin julian --quiet -- net anchor --log-dir "$TMP_DIR/logs" 2>&1 || true)"
check_contains "$anchor_flagged" "\"schema\"" "net anchor --log-dir output"

anchor_positional="$(cargo run --features net --bin julian --quiet -- net anchor "$TMP_DIR/logs" 2>&1 || true)"
check_contains "$anchor_positional" "\"schema\"" "net anchor positional output"

echo "[7/10] verify token mode flags exposed"
net_start_help="$(cargo run --features net --bin julian --quiet -- net start --help 2>&1 || true)"
check_contains "$net_start_help" "--token-mode" "net start token-mode flag"
check_contains "$net_start_help" "--token-oracle" "net start token-oracle flag"

echo "[8/10] verify claim manifest generation works"
REGISTRY="$TMP_DIR/registry.json"
SNAPSHOT="$TMP_DIR/snapshot.json"
CLAIMS="$TMP_DIR/claims.json"
cat >"$REGISTRY" <<'JSON'
{
  "accounts": {
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=": {"balance": 1000, "stake": 250, "slashed": false},
    "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=": {"balance": 750, "stake": 200, "slashed": false},
    "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=": {"balance": 0, "stake": 0, "slashed": true}
  }
}
JSON

cargo run --features net --bin julian --quiet -- \
  stake snapshot --registry "$REGISTRY" --height 7 --output "$SNAPSHOT" >/dev/null
cargo run --features net --bin julian --quiet -- \
  stake claims --snapshot "$SNAPSHOT" --output "$CLAIMS" --amount-source total >/dev/null

claims_root="$(python3 - "$CLAIMS" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert "merkle_root" in doc and doc["merkle_root"].startswith("0x")
assert doc.get("claim_count", 0) > 0
print(doc["merkle_root"])
PY
)"
check_contains "$claims_root" "0x" "claim manifest root"

echo "[9/10] verify phase-3 scripts are present"
[[ -x scripts/build_migration_claims.sh ]] || fail "scripts/build_migration_claims.sh missing"
[[ -x scripts/build_powerhouse_token_artifact.sh ]] || fail "scripts/build_powerhouse_token_artifact.sh missing"
[[ -x scripts/deploy_powerhouse_token.py ]] || fail "scripts/deploy_powerhouse_token.py missing"

echo "[10/10] verify deploy script help"
deploy_help="$(python3 scripts/deploy_powerhouse_token.py --help 2>&1 || true)"
check_contains "$deploy_help" "--migration-root" "deploy script options"

echo "verify_migration_contract: PASS"
