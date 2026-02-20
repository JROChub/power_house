#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

CARGO_BIN=${CARGO_BIN:-cargo}
RUN_NET_SMOKE=${RUN_NET_SMOKE:-1}
WORK_DIR=${MIGRATION_DRY_RUN_DIR:-"$ROOT_DIR/logs/token_migration_dry_run"}

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"
REGISTRY_JSON="$WORK_DIR/stake_registry.json"
SNAPSHOT_JSON="$WORK_DIR/migration_snapshot.json"
MIGRATION_ANCHOR_JSON="$WORK_DIR/migration_anchor.json"
LEDGER_DIR="$WORK_DIR/ledger"
ANCHOR_TXT="$WORK_DIR/anchor.txt"
PROOF_JSON="$WORK_DIR/proof.json"

mkdir -p "$LEDGER_DIR"

cat >"$REGISTRY_JSON" <<'JSON'
{
  "accounts": {
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=": {"balance": 1000, "stake": 250, "slashed": false},
    "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=": {"balance": 750, "stake": 200, "slashed": false},
    "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC=": {"balance": 300, "stake": 40, "slashed": true}
  }
}
JSON

echo "[1/6] build + tests"
"$CARGO_BIN" test

echo "[2/6] deterministic stake snapshot"
"$CARGO_BIN" run --features net --bin julian --quiet -- \
  stake snapshot --registry "$REGISTRY_JSON" --height 1 --output "$SNAPSHOT_JSON"

echo "[3/6] governance migration proposal anchor"
"$CARGO_BIN" run --features net --bin julian --quiet -- \
  governance propose-migration \
    --snapshot-height 1 \
    --token-contract "0x0000000000000000000000000000000000000001" \
    --conversion-ratio 1 \
    --treasury-mint 0 \
    --log-dir "$LEDGER_DIR" \
    --node-id "dry-run" \
    --quorum 1 \
    --output "$MIGRATION_ANCHOR_JSON"

echo "[4/6] produce baseline ledger anchor + proof"
"$CARGO_BIN" run --bin julian --quiet -- node run dry-run "$LEDGER_DIR" "$ANCHOR_TXT"
"$CARGO_BIN" run --bin julian --quiet -- node prove "$LEDGER_DIR" 0 0 "$PROOF_JSON"

echo "[5/6] verify anchor proof"
"$CARGO_BIN" run --bin julian --quiet -- node verify-proof "$ANCHOR_TXT" "$PROOF_JSON"

echo "[6/6] optional smoke net (--with-migration)"
if [[ "$RUN_NET_SMOKE" == "1" ]]; then
  ./scripts/smoke_net.sh --with-migration
else
  echo "RUN_NET_SMOKE=0, skipping network smoke"
fi

echo "token_migration_dry_run: PASS"
echo "artifacts:"
echo "  snapshot: $SNAPSHOT_JSON"
echo "  migration_anchor: $MIGRATION_ANCHOR_JSON"
