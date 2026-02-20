#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

CARGO_BIN=${CARGO_BIN:-cargo}
RUN_NET_SMOKE=${RUN_NET_SMOKE:-1}
RUN_TOKEN_BUILD=${RUN_TOKEN_BUILD:-0}
WORK_DIR=${MIGRATION_DRY_RUN_DIR:-"$ROOT_DIR/logs/token_migration_dry_run"}

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"
REGISTRY_JSON="$WORK_DIR/stake_registry.json"
FINALIZE_DIR="$WORK_DIR/finalize"
SNAPSHOT_JSON="$FINALIZE_DIR/migration_snapshot.json"
CLAIMS_JSON="$FINALIZE_DIR/migration_claims.json"
APPLY_STATE_JSON="$FINALIZE_DIR/migration_apply_state.json"
MIGRATION_ANCHOR_JSON="$FINALIZE_DIR/migration_anchor.json"
MIGRATION_ENVELOPE_JSON="$FINALIZE_DIR/migration_anchor_envelope.json"
BURN_OUTBOX_JSONL="$WORK_DIR/token_burn_outbox.jsonl"
BURN_STATE_JSON="$WORK_DIR/token_burn_exec_state.json"
TOKEN_ARTIFACT_JSON="$WORK_DIR/PowerHouseToken.json"
LEDGER_DIR="$WORK_DIR/ledger"
ANCHOR_TXT="$WORK_DIR/anchor.txt"
PROOF_JSON="$WORK_DIR/proof.json"

mkdir -p "$LEDGER_DIR" "$FINALIZE_DIR"

cat >"$REGISTRY_JSON" <<'JSON'
{
  "accounts": {
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=": {"balance": 1000, "stake": 250, "slashed": false},
    "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=": {"balance": 750, "stake": 200, "slashed": false},
    "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=": {"balance": 300, "stake": 40, "slashed": true}
  }
}
JSON

echo "[1/11] build + tests"
"$CARGO_BIN" test

echo "[2/11] run deterministic migration finalize"
"$CARGO_BIN" run --features net --bin julian --quiet -- \
  migration finalize \
    --registry "$REGISTRY_JSON" \
    --height 1 \
    --log-dir "$LEDGER_DIR" \
    --output-dir "$FINALIZE_DIR" \
    --token-contract "native://julian" \
    --conversion-ratio 1 \
    --treasury-mint 0 \
    --amount-source total \
    --node-id "dry-run" \
    --quorum 1 \
    --apply-state "$APPLY_STATE_JSON" \
    --allow-unfrozen \
    --force

echo "[3/11] verify migration state integrity"
"$CARGO_BIN" run --features net --bin julian --quiet -- \
  migration verify-state \
    --registry "$REGISTRY_JSON" \
    --claims "$CLAIMS_JSON" \
    --state "$APPLY_STATE_JSON" \
    --require-complete

echo "[4/11] create synthetic native burn intent"
cat >"$BURN_OUTBOX_JSONL" <<'JSONL'
{"schema":"mfenx.powerhouse.token-burn-intent.v1","token_contract":"native://julian","pubkey_b64":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=","reason":"dry-run synthetic"}
JSONL

echo "[5/11] execute native burn intents"
"$CARGO_BIN" run --features net --bin julian --quiet -- \
  migration execute-burn-intents \
    --registry "$REGISTRY_JSON" \
    --outbox "$BURN_OUTBOX_JSONL" \
    --state "$BURN_STATE_JSON"

echo "[6/11] verify anchor artifact from finalize"
python3 - "$MIGRATION_ANCHOR_JSON" "$MIGRATION_ENVELOPE_JSON" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], "r", encoding="utf-8"))
if "migration_anchor" not in doc:
    raise SystemExit("missing migration_anchor object in migration artifact")
anchor = doc.get("anchor_json")
if not isinstance(anchor, dict):
    raise SystemExit("missing anchor_json object in migration artifact")
if "schema" not in anchor:
    raise SystemExit("anchor_json missing schema")
json.dump(anchor, open(sys.argv[2], "w", encoding="utf-8"), indent=2)
PY

echo "[7/11] produce baseline ledger anchor + proof"
"$CARGO_BIN" run --bin julian --quiet -- node run dry-run "$LEDGER_DIR" "$ANCHOR_TXT"
"$CARGO_BIN" run --bin julian --quiet -- node prove "$LEDGER_DIR" 0 0 "$PROOF_JSON"

echo "[8/11] verify anchor proof"
"$CARGO_BIN" run --bin julian --quiet -- node verify-proof "$ANCHOR_TXT" "$PROOF_JSON"

echo "[9/11] check idempotent burn executor"
"$CARGO_BIN" run --features net --bin julian --quiet -- \
  migration execute-burn-intents \
    --registry "$REGISTRY_JSON" \
    --outbox "$BURN_OUTBOX_JSONL" \
    --state "$BURN_STATE_JSON"

echo "[10/11] optional token artifact build (RUN_TOKEN_BUILD=1)"
if [[ "$RUN_TOKEN_BUILD" == "1" ]]; then
  OUT_FILE="$TOKEN_ARTIFACT_JSON" ./scripts/build_powerhouse_token_artifact.sh
else
  echo "RUN_TOKEN_BUILD=0, skipping solidity compile"
fi

echo "[11/11] optional smoke net (--with-migration)"
if [[ "$RUN_NET_SMOKE" == "1" ]]; then
  ./scripts/smoke_net.sh --with-migration
else
  echo "RUN_NET_SMOKE=0, skipping network smoke"
fi

echo "token_migration_dry_run: PASS"
echo "artifacts:"
echo "  snapshot: $SNAPSHOT_JSON"
echo "  claims: $CLAIMS_JSON"
echo "  apply_state: $APPLY_STATE_JSON"
echo "  burn_outbox: $BURN_OUTBOX_JSONL"
echo "  burn_state: $BURN_STATE_JSON"
echo "  migration_anchor: $MIGRATION_ANCHOR_JSON"
echo "  migration_envelope: $MIGRATION_ENVELOPE_JSON"
if [[ "$RUN_TOKEN_BUILD" == "1" ]]; then
  echo "  token_artifact: $TOKEN_ARTIFACT_JSON"
fi
