#!/usr/bin/env python3

from __future__ import annotations

import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / "scripts" / "generate_rpc_cluster.py"
BINARY = ROOT / "target" / "debug" / "julian"


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="powerhouse-cluster-test-") as temp:
        output = Path(temp) / "bundle"
        result = subprocess.run(
            [
                str(GENERATOR),
                "--output",
                str(output),
                "--binary",
                str(BINARY),
                "--host",
                "10.42.0.11",
                "--host",
                "10.42.0.12",
                "--host",
                "validator-3.internal",
                "--fund",
                "0x4a62316623ad457f02cdc5d997ded67a383ec569:1000000",
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        summary = json.loads(result.stdout)
        assert summary["validators"] == 3
        assert summary["quorum"] == 2

        manifest = json.loads((output / "cluster-manifest.json").read_text())
        policy = json.loads((output / "native-validators.json").read_text())
        registry = json.loads((output / "stake_registry.json").read_text())
        validator_registry = json.loads(
            (output / "validator-registry.json").read_text()
        )
        assert manifest["chain_id"] == 177155
        assert manifest["validator_registry"] == "validator-registry.json"
        assert len({item["peer_id"] for item in manifest["validators"]}) == 3
        assert len({item["public_key_b64"] for item in manifest["validators"]}) == 3
        assert policy["allowlist"] == [
            item["public_key_b64"] for item in manifest["validators"]
        ]
        assert validator_registry["chain_id"] == 177155
        assert len(validator_registry["registrations"]) == 3
        subprocess.run(
            [
                str(BINARY),
                "validator-registry",
                "verify",
                str(output / "validator-registry.json"),
                "--policy",
                str(output / "native-validators.json"),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        assert (
            registry["accounts"][
                "0x4a62316623ad457f02cdc5d997ded67a383ec569"
            ]["balance"]
            == 1_000_000
        )

        node_one = (output / "powerhouse-validator-1.env").read_text()
        assert "/ip4/10.42.0.12/tcp/7001/p2p/" in node_one
        assert "/dns4/validator-3.internal/tcp/7001/p2p/" in node_one
        assert "validator-1.key" in node_one

        for index in range(1, 4):
            key = output / f"validator-{index}.key"
            assert key.stat().st_size == 32
            assert stat.S_IMODE(key.stat().st_mode) == 0o600

        overwrite = subprocess.run(
            [
                str(GENERATOR),
                "--output",
                str(output),
                "--binary",
                str(BINARY),
                "--host",
                "10.42.0.11",
                "--host",
                "10.42.0.12",
                "--host",
                "10.42.0.13",
            ],
            capture_output=True,
            text=True,
        )
        assert overwrite.returncode != 0
        assert "refusing to overwrite" in overwrite.stderr

    print("test_generate_rpc_cluster: PASS")


if __name__ == "__main__":
    os.chdir(ROOT)
    main()
