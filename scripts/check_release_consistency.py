#!/usr/bin/env python3
"""Reject a release when active public version or network metadata drifts."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
ACTIVE_DOCS = [
    "JULIAN_PROTOCOL.md",
    "docs/README.md",
    "docs/committed_workload.md",
    "docs/hyperscale_proof.md",
    "docs/incident_response.md",
    "docs/load_testing.md",
    "docs/network_roadmap.md",
    "docs/node_operator.md",
    "docs/ops.md",
    "docs/orbital_observatory.md",
    "docs/pha_spec.md",
    "docs/prior_art_review.md",
    "docs/production_rpc_deployment.md",
    "docs/provenance_security.md",
    "docs/research_protocol.md",
    "docs/rootprint.md",
    "docs/rpc_operations.md",
    "docs/sdk.md",
    "docs/security_model.md",
    "docs/sextillion_proof.md",
    "docs/sparse_record.md",
    "docs/testnet_mainnet.md",
    "docs/verification_guide.md",
    "sdk/python/README.md",
]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def cargo_lock_version() -> str | None:
    lock = tomllib.loads(read("Cargo.lock"))
    for package in lock["package"]:
        if package["name"] == "power_house":
            return package["version"]
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected-tag")
    args = parser.parse_args()

    errors: list[str] = []
    cargo = tomllib.loads(read("Cargo.toml"))
    version = cargo["package"]["version"]
    release = f"v{version}"

    require(errors, cargo_lock_version() == version, "Cargo.lock package version differs")

    python_project = tomllib.loads(read("sdk/python/pyproject.toml"))
    require(
        errors,
        python_project["project"]["version"] == version,
        "Python package version differs",
    )
    require(
        errors,
        f'__version__ = "{version}"' in read("sdk/python/power_house/__init__.py"),
        "Python runtime version differs",
    )
    require(
        errors,
        f"Current release: **{release}**" in read("README.md"),
        "README current release is missing or stale",
    )
    require(
        errors,
        read("RELEASE_NOTES.md").startswith(f"# Release Notes\n\n## {release} "),
        "release notes do not start with the current release",
    )

    for path in ACTIVE_DOCS:
        require(errors, release in read(path) or version in read(path), f"{path} is stale")

    website = read("publicpower/index.html")
    require(errors, website.count(release) >= 2, "website release labels are stale")
    require(
        errors,
        f"ARG POWER_HOUSE_VERSION={version}" in read("Dockerfile"),
        "Docker image label version differs",
    )
    require(
        errors,
        f"ghcr.io/jrochub/power_house:{version}" in read("docker-compose.yml"),
        "Docker Compose image tag differs",
    )

    network = json.loads(read("configs/network.json"))
    benchmark = json.loads(read(f"benchmarks/v{version}/rpc-report.json"))
    require(errors, network.get("release") == version, "network metadata version differs")
    require(errors, benchmark.get("release") == version, "RPC benchmark version differs")
    require(errors, network.get("chainId") == 177155, "network chain ID differs")
    require(
        errors,
        network.get("rpc", {}).get("name") == "LAX MFENX RPC",
        "public RPC name differs",
    )
    require(
        errors,
        network.get("rpc", {}).get("chainListUrl")
        == "https://rpc.mfenx.com",
        "ChainList RPC URL differs",
    )

    active_paths = [
        "README.md",
        "publicpower/index.html",
        "docs/README.md",
        "docs/ops.md",
        "docs/production_rpc_deployment.md",
        "docs/rpc_operations.md",
        "configs/network.json",
        "scripts/digitalocean_rpc_preflight.sh",
    ]
    for path in active_paths:
        require(
            errors,
            not re.search(r"\bDigitalOcean RPC\b", read(path), re.IGNORECASE),
            f"{path} contains the retired public RPC name",
        )

    if args.expected_tag:
        require(
            errors,
            args.expected_tag == release,
            f"tag {args.expected_tag} does not match {release}",
        )

    if errors:
        for error in errors:
            print(f"RELEASE GATE FAIL: {error}", file=sys.stderr)
        return 1
    print(f"RELEASE GATE PASS: Power House {release}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
