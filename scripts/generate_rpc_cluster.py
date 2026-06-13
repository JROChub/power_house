#!/usr/bin/env python3
"""Generate a sealed three-validator Power-House RPC deployment bundle."""

from __future__ import annotations

import argparse
import ipaddress
import json
import os
from pathlib import Path
import re
import secrets
import stat
import subprocess
import sys
import time


VALIDATOR_COUNT = 3
DEFAULT_CHAIN_ID = 177155
DEFAULT_REGIONS = ["nyc3", "sfo3", "ams3"]
EVM_ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate keys and configuration for a three-validator RPC cluster."
    )
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path("target/release/julian"),
        help="network-enabled julian binary",
    )
    parser.add_argument(
        "--host",
        action="append",
        required=True,
        help="private validator IP or DNS name; specify exactly three times",
    )
    parser.add_argument("--chain-id", type=int, default=DEFAULT_CHAIN_ID)
    parser.add_argument(
        "--fund",
        action="append",
        default=[],
        metavar="ADDRESS:TOKENS",
        help="genesis whole-token balance; repeatable",
    )
    return parser.parse_args()


def fail(message: str) -> None:
    raise SystemExit(message)


def parse_funding(entries: list[str]) -> dict[str, dict[str, int | bool]]:
    accounts: dict[str, dict[str, int | bool]] = {}
    for entry in entries:
        address, separator, raw_amount = entry.partition(":")
        if not separator or not EVM_ADDRESS.fullmatch(address):
            fail(f"invalid --fund value: {entry}")
        try:
            amount = int(raw_amount)
        except ValueError:
            fail(f"invalid funding amount: {raw_amount}")
        if amount < 0 or amount > (2**64 - 1):
            fail(f"funding amount outside u64 range: {raw_amount}")
        normalized = address.lower()
        previous = accounts.get(normalized, {}).get("balance", 0)
        total = int(previous) + amount
        if total > (2**64 - 1):
            fail(f"combined funding exceeds u64 for {normalized}")
        accounts[normalized] = {"balance": total, "slashed": False, "stake": 0}
    return accounts


def multiaddr_host(host: str) -> str:
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        if not host or any(char.isspace() for char in host):
            fail(f"invalid validator host: {host!r}")
        return f"/dns4/{host}"
    return f"/ip{address.version}/{address}"


def inspect_key(binary: Path, key_path: Path) -> dict[str, str]:
    result = subprocess.run(
        [str(binary), "key-info", str(key_path), "--json"],
        check=True,
        capture_output=True,
        text=True,
    )
    info = json.loads(result.stdout)
    if set(info) != {"peer_id", "public_key_b64"}:
        fail("key-info returned an unexpected payload")
    return info


def create_validator_registry(
    binary: Path,
    output: Path,
    hosts: list[str],
    keys: list[dict[str, str]],
    chain_id: int,
) -> None:
    issued_at = int(time.time())
    valid_until = issued_at + 365 * 24 * 60 * 60
    registrations = []
    for index, host in enumerate(hosts):
        registration = output / f"validator-{index + 1}.registration.json"
        p2p_address = (
            f"{multiaddr_host(host)}/tcp/7001/p2p/{keys[index]['peer_id']}"
        )
        subprocess.run(
            [
                str(binary),
                "validator-registry",
                "create",
                "--key",
                str(output / f"validator-{index + 1}.key"),
                "--node-id",
                f"validator-{index + 1}",
                "--operator",
                "MFENX LLC",
                "--region",
                DEFAULT_REGIONS[index],
                "--p2p-address",
                p2p_address,
                "--metrics-url",
                f"http://{host}:9100/metrics",
                "--system-metrics-url",
                f"http://{host}:9101/metrics",
                "--chain-id",
                str(chain_id),
                "--issued-at",
                str(issued_at),
                "--valid-until",
                str(valid_until),
                "--output",
                str(registration),
            ],
            check=True,
        )
        os.chmod(registration, 0o640)
        registrations.append(registration)

    command = [
        str(binary),
        "validator-registry",
        "assemble",
        "--policy",
        str(output / "native-validators.json"),
        "--chain-id",
        str(chain_id),
        "--output",
        str(output / "validator-registry.json"),
    ]
    for registration in registrations:
        command.extend(["--registration", str(registration)])
    subprocess.run(command, check=True)
    os.chmod(output / "validator-registry.json", 0o640)


def write_private(path: Path, data: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(data)


def write_text(path: Path, contents: str, mode: int = 0o640) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(contents)


def render_common(chain_id: int) -> str:
    return "\n".join(
        [
            "PH_BROADCAST_INTERVAL=1500",
            "PH_QUORUM=2",
            "PH_ATTESTATION_QUORUM=2",
            "PH_CHECKPOINT_INTERVAL=60",
            "PH_POLICY=/etc/powerhouse/native-validators.json",
            f"PH_EVM_CHAIN_ID={chain_id}",
            "PH_EVM_RPC_LISTEN=127.0.0.1:8545",
            "PH_RPC_HEALTH_URL=http://127.0.0.1:8545/healthz",
            "PH_METRICS_ADDR=0.0.0.0:9100",
            "PH_METRICS_URL=http://127.0.0.1:9100/metrics",
            "PH_BLOB_LISTEN=127.0.0.1:8181",
            "PH_HEALTH_URL=http://127.0.0.1:8181/healthz",
            "PH_BLOB_MAX_CONCURRENCY=128",
            "PH_BLOB_REQUEST_TIMEOUT_MS=10000",
            "PH_MAX_BLOB_BYTES=5242880",
            "PH_BLOB_RETENTION_DAYS=30",
            "PH_METRICS_STALL_MINUTES=20",
            "PH_AUTO_RECOVERY=1",
            "PH_RECOVERY_COOLDOWN_SECONDS=900",
            "PH_BACKUP_DIR=/var/backups/powerhouse",
            "PH_BACKUP_RETENTION_DAYS=14",
            "PH_RELEASE_ROOT=/opt/powerhouse/releases",
            "",
        ]
    )


def render_node(index: int, hosts: list[str], keys: list[dict[str, str]]) -> str:
    node = f"validator-{index + 1}"
    bootstraps = []
    for peer_index, host in enumerate(hosts):
        if peer_index == index:
            continue
        bootstraps.append(
            f"{multiaddr_host(host)}/tcp/7001/p2p/{keys[peer_index]['peer_id']}"
        )
    base = f"/var/lib/powerhouse/{node}"
    return "\n".join(
        [
            f"PH_NODE_ID={node}",
            f"PH_LOG_DIR={base}/logs",
            "PH_LISTEN=/ip4/0.0.0.0/tcp/7001",
            f"PH_KEY=/etc/powerhouse/{node}.key",
            f"PH_BLOB_DIR={base}",
            f'PH_BOOTSTRAPS="{" ".join(bootstraps)}"',
            f"PH_SERVICE_NAME=powerhouse-node@{node}.service",
            (
                f'PH_BACKUP_SOURCES="{base} '
                '/etc/powerhouse/native-validators.json"'
            ),
            "",
        ]
    )


def main() -> int:
    args = parse_args()
    if len(args.host) != VALIDATOR_COUNT:
        fail(f"--host must be specified exactly {VALIDATOR_COUNT} times")
    if args.chain_id <= 0 or args.chain_id > (2**64 - 1):
        fail("--chain-id must be between 1 and 2^64-1")

    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        fail(f"executable julian binary not found: {binary}")

    output = args.output.resolve()
    if output.exists():
        fail(f"refusing to overwrite existing output directory: {output}")
    output.mkdir(parents=True, mode=0o700)
    os.chmod(output, 0o700)

    key_info: list[dict[str, str]] = []
    for index in range(VALIDATOR_COUNT):
        key_path = output / f"validator-{index + 1}.key"
        write_private(key_path, secrets.token_bytes(32))
        key_info.append(inspect_key(binary, key_path))

    policy = {
        "allowlist": [entry["public_key_b64"] for entry in key_info],
        "backend": "static",
    }
    write_text(
        output / "native-validators.json",
        json.dumps(policy, indent=2, sort_keys=True) + "\n",
    )
    create_validator_registry(binary, output, args.host, key_info, args.chain_id)
    write_text(output / "powerhouse-common.env", render_common(args.chain_id))

    for index in range(VALIDATOR_COUNT):
        write_text(
            output / f"powerhouse-validator-{index + 1}.env",
            render_node(index, args.host, key_info),
        )

    registry = {"accounts": parse_funding(args.fund)}
    write_text(
        output / "stake_registry.json",
        json.dumps(registry, indent=2, sort_keys=True) + "\n",
    )
    manifest = {
        "chain_id": args.chain_id,
        "hosts": args.host,
        "quorum": 2,
        "validator_registry": "validator-registry.json",
        "validators": [
            {
                "name": f"validator-{index + 1}",
                **entry,
            }
            for index, entry in enumerate(key_info)
        ],
    }
    write_text(
        output / "cluster-manifest.json",
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
    )
    write_text(
        output / "README.txt",
        (
            "CONFIDENTIAL OPERATOR BUNDLE\n\n"
            "The validator-*.key files control consensus identities. Keep this directory "
            "offline, encrypted, and backed up. Never commit or upload it as a public "
            "artifact. Copy each key only to its matching validator with mode 0600.\n"
        ),
        mode=0o600,
    )

    for path in output.iterdir():
        expected = 0o600 if path.name.endswith(".key") or path.name == "README.txt" else 0o640
        if stat.S_IMODE(path.stat().st_mode) != expected:
            fail(f"unexpected permissions on {path}")

    print(
        json.dumps(
            {
                "chain_id": args.chain_id,
                "output": str(output),
                "quorum": 2,
                "validators": VALIDATOR_COUNT,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
