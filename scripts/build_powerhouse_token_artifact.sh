#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
cd "$ROOT_DIR"

SOLC_BIN=${SOLC:-solc}
NPM_BIN=${NPM_BIN:-npm}
CONTRACT_PATH=${CONTRACT_PATH:-contracts/PowerHouseToken.sol}
CONTRACT_NAME=${CONTRACT_NAME:-PowerHouseToken}
OZ_VERSION=${OZ_VERSION:-5.0.2}
NODE_DEPS_DIR=${NODE_DEPS_DIR:-$ROOT_DIR/.soldeps}
OUT_DIR=${OUT_DIR:-$ROOT_DIR/artifacts}
OUT_FILE=${OUT_FILE:-$OUT_DIR/PowerHouseToken.json}

command -v "$SOLC_BIN" >/dev/null 2>&1 || {
  echo "solc not found (set SOLC=/path/to/solc)" >&2
  exit 1
}
command -v "$NPM_BIN" >/dev/null 2>&1 || {
  echo "npm not found (required to fetch @openzeppelin/contracts)" >&2
  exit 1
}

mkdir -p "$NODE_DEPS_DIR" "$OUT_DIR"
if [[ ! -f "$NODE_DEPS_DIR/package.json" ]]; then
  cat >"$NODE_DEPS_DIR/package.json" <<'JSON'
{
  "name": "powerhouse-soldeps",
  "private": true,
  "version": "1.0.0"
}
JSON
fi
if [[ ! -d "$NODE_DEPS_DIR/node_modules/@openzeppelin/contracts" ]]; then
  "$NPM_BIN" --prefix "$NODE_DEPS_DIR" install --silent --no-audit --no-fund "@openzeppelin/contracts@${OZ_VERSION}"
fi

TMP_JSON="$(mktemp)"
trap 'rm -f "$TMP_JSON"' EXIT

"$SOLC_BIN" \
  --base-path "$ROOT_DIR" \
  --include-path "$NODE_DEPS_DIR/node_modules" \
  --optimize \
  --combined-json abi,bin \
  "$CONTRACT_PATH" > "$TMP_JSON"

SOLC_VERSION="$($SOLC_BIN --version 2>/dev/null | awk 'NR==2 {print; found=1} END {if (!found) print ""}' | xargs)"
if [[ -z "$SOLC_VERSION" ]]; then
  SOLC_VERSION="$($SOLC_BIN --version 2>/dev/null | head -n 1 | xargs)"
fi
export TMP_JSON OUT_FILE CONTRACT_NAME CONTRACT_PATH SOLC_VERSION OZ_VERSION

python3 - <<'PY'
import json
import os
import sys

in_path = os.environ["TMP_JSON"]
out_path = os.environ["OUT_FILE"]
contract_name = os.environ["CONTRACT_NAME"]
contract_path = os.environ["CONTRACT_PATH"]
solc_version = os.environ["SOLC_VERSION"]
oz_version = os.environ["OZ_VERSION"]

with open(in_path, "r", encoding="utf-8") as f:
    compiled = json.load(f)

contracts = compiled.get("contracts", {})
selected_key = None
for key in contracts:
    if key.endswith(f":{contract_name}"):
        selected_key = key
        break

if selected_key is None:
    print(f"failed to find {contract_name} in solc output", file=sys.stderr)
    sys.exit(1)

entry = contracts[selected_key]
bytecode = entry.get("bin", "")
if not bytecode:
    print("compiled bytecode is empty", file=sys.stderr)
    sys.exit(1)

abi_raw = entry.get("abi", [])
if isinstance(abi_raw, str):
    abi = json.loads(abi_raw)
elif isinstance(abi_raw, list):
    abi = abi_raw
else:
    print("unexpected abi format in compiler output", file=sys.stderr)
    sys.exit(1)

artifact = {
    "schema": "mfenx.powerhouse.solidity-artifact.v1",
    "contract_name": contract_name,
    "source_path": contract_path,
    "solc_version": solc_version,
    "openzeppelin_version": oz_version,
    "abi": abi,
    "bytecode": "0x" + bytecode,
}

with open(out_path, "w", encoding="utf-8") as f:
    json.dump(artifact, f, indent=2)

print(f"wrote artifact: {out_path}")
PY
