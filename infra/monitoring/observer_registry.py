#!/usr/bin/env python3
"""Verify signed public observer registrations and publish health state."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener


STATE_SCHEMA = "power-house-observer-registry-health-v1"
IDENTITY_METRIC = re.compile(
    r'^powerhouse_node_identity\{(?P<labels>.+)\}\s+(?P<value>[0-9.eE+-]+)$'
)
LABEL = re.compile(r'([a-zA-Z_][a-zA-Z0-9_]*)="((?:\\.|[^"])*)"')
SIMPLE_METRIC = re.compile(
    r"^(?P<name>[a-zA-Z_:][a-zA-Z0-9_:]*)\s+(?P<value>[0-9.eE+-]+)$"
)
MAX_METRICS_BYTES = 2 * 1024 * 1024
MAX_REGISTRY_BYTES = 2 * 1024 * 1024


class RejectRedirects(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError(f"metrics endpoint redirect rejected: HTTP {code}")


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--registry",
        type=Path,
        default=Path(
            os.environ.get(
                "OBSERVER_REGISTRY_PATH", "/etc/powerhouse/observer-registry.json"
            )
        ),
    )
    parser.add_argument("--binary", type=Path, default=Path("/usr/local/bin/julian"))
    parser.add_argument(
        "--state",
        type=Path,
        default=Path("/var/lib/powerhouse/monitoring/observer-registry-state.json"),
    )
    parser.add_argument(
        "--observer-discovery",
        type=Path,
        default=Path("/etc/prometheus/file_sd/powerhouse-observers.json"),
    )
    parser.add_argument(
        "--registry-url",
        default=os.environ.get("OBSERVER_REGISTRY_URL"),
    )
    parser.add_argument("--timeout", type=float, default=5.0)
    return parser.parse_args()


def atomic_json(path: Path, value: object, mode: int = 0o640) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def not_configured_state(generated_at: str) -> dict:
    return {
        "schema": STATE_SCHEMA,
        "configured": False,
        "generated_at": generated_at,
        "registry_verified": False,
        "observers_total": 0,
        "observers_healthy": 0,
        "observer_connections": 0,
        "observers": [],
        "error": "observer registry not configured",
    }


def verify_registry(args: argparse.Namespace, now: int, registry: Path | None = None) -> dict:
    process = subprocess.run(
        [
            str(args.binary),
            "observer-registry",
            "verify",
            str(registry or args.registry),
            "--now",
            str(now),
            "--json",
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=max(args.timeout * 2, 10),
    )
    if process.returncode:
        detail = process.stderr.strip() or process.stdout.strip() or "verification failed"
        raise RuntimeError(detail)
    verified = json.loads(process.stdout)
    if verified.get("verified") is not True:
        raise RuntimeError("observer verifier did not return verified=true")
    return verified


def synchronize_registry(args: argparse.Namespace, now: int) -> None:
    if not args.registry_url:
        return
    request = Request(
        args.registry_url,
        headers={"User-Agent": "power-house-observer-registry-sync/1"},
    )
    with build_opener(RejectRedirects).open(request, timeout=args.timeout) as response:
        if response.status != 200:
            raise RuntimeError(f"registry source returned HTTP {response.status}")
        payload = response.read(MAX_REGISTRY_BYTES + 1)
    if len(payload) > MAX_REGISTRY_BYTES:
        raise RuntimeError("registry source exceeds 2 MiB")
    try:
        value = json.loads(payload)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"registry source returned invalid JSON: {exc}") from exc
    args.registry.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{args.registry.name}.sync.", dir=args.registry.parent
    )
    os.close(descriptor)
    candidate = Path(temporary)
    try:
        atomic_json(candidate, value)
        verify_registry(args, now, candidate)
        os.replace(candidate, args.registry)
        os.chmod(args.registry, 0o640)
    finally:
        candidate.unlink(missing_ok=True)


def decode_label(value: str) -> str:
    return (
        value.replace(r"\\", "\0")
        .replace(r"\"", '"')
        .replace(r"\n", "\n")
        .replace("\0", "\\")
    )


def parse_metrics(body: str) -> tuple[dict[str, str] | None, dict[str, float]]:
    identity = None
    metrics: dict[str, float] = {}
    for line in body.splitlines():
        if not line or line.startswith("#"):
            continue
        identity_match = IDENTITY_METRIC.fullmatch(line)
        if identity_match:
            labels = {
                name: decode_label(value)
                for name, value in LABEL.findall(identity_match.group("labels"))
            }
            if float(identity_match.group("value")) == 1:
                identity = labels
            continue
        metric_match = SIMPLE_METRIC.fullmatch(line)
        if metric_match:
            metrics[metric_match.group("name")] = float(metric_match.group("value"))
    return identity, metrics


def fetch(url: str, timeout: float) -> str:
    request = Request(url, headers={"User-Agent": "power-house-observer-registry/1"})
    with build_opener(RejectRedirects).open(request, timeout=timeout) as response:
        if response.status != 200:
            raise RuntimeError(f"HTTP {response.status}")
        body = response.read(MAX_METRICS_BYTES + 1)
        if len(body) > MAX_METRICS_BYTES:
            raise RuntimeError("metrics response exceeds 2 MiB")
        return body.decode("utf-8", "replace")


def check_observer(registration: dict, timeout: float) -> dict:
    result = {
        "node_id": registration["node_id"],
        "operator": registration["operator"],
        "region": registration["region"],
        "peer_id": registration["peer_id"],
        "public_key_b64": registration["public_key_b64"],
        "identity_verified": False,
        "metrics_reachable": False,
        "system_metrics_reachable": registration.get("system_metrics_url") is None,
        "peer_links": 0,
        "healthy": False,
        "error": None,
    }
    errors = []
    try:
        body = fetch(registration["metrics_url"], timeout)
        result["metrics_reachable"] = True
        identity, metrics = parse_metrics(body)
        expected = {
            "node_id": registration["node_id"],
            "peer_id": registration["peer_id"],
            "public_key_b64": registration["public_key_b64"],
            "chain_id": str(registration["chain_id"]),
        }
        if identity != expected:
            errors.append("live identity metric does not match signed observer registration")
        else:
            result["identity_verified"] = True
        peers = metrics.get("powerhouse_connected_peers")
        if peers is None or peers < 0 or not peers.is_integer():
            errors.append("connected peer metric is missing or invalid")
        else:
            result["peer_links"] = int(peers)
    except Exception as exc:
        errors.append(f"observer metrics unavailable: {exc}")

    system_url = registration.get("system_metrics_url")
    if system_url:
        try:
            fetch(system_url, timeout)
            result["system_metrics_reachable"] = True
        except Exception as exc:
            errors.append(f"system metrics unavailable: {exc}")

    result["healthy"] = (
        result["identity_verified"]
        and result["metrics_reachable"]
        and result["system_metrics_reachable"]
    )
    if errors:
        result["error"] = "; ".join(errors)
    return result


def discovery_entry(registration: dict) -> dict:
    parsed = urlsplit(registration["metrics_url"])
    return {
        "targets": [parsed.netloc],
        "labels": {
            "node": registration["node_id"],
            "operator": registration["operator"],
            "region": registration["region"],
            "peer_id": registration["peer_id"],
            "public_key_b64": registration["public_key_b64"],
            "role": "observer",
        },
    }


def reconcile(args: argparse.Namespace) -> dict:
    generated_at = datetime.now(timezone.utc).isoformat()
    now = int(time.time())
    if args.registry_url:
        synchronize_registry(args, now)
    if not args.registry.exists():
        state = not_configured_state(generated_at)
        atomic_json(args.state, state)
        atomic_json(args.observer_discovery, [], mode=0o644)
        return state

    try:
        verified = verify_registry(args, now)
    except Exception as exc:
        state = {
            "schema": STATE_SCHEMA,
            "configured": True,
            "generated_at": generated_at,
            "registry_verified": False,
            "observers_total": 0,
            "observers_healthy": 0,
            "observer_connections": 0,
            "observers": [],
            "error": str(exc),
        }
        atomic_json(args.state, state)
        raise

    registrations = verified["registrations"]
    with ThreadPoolExecutor(max_workers=min(len(registrations), 16)) as executor:
        observers = list(
            executor.map(lambda item: check_observer(item, args.timeout), registrations)
        )

    state = {
        "schema": STATE_SCHEMA,
        "chain_id": verified["chain_id"],
        "configured": True,
        "generated_at": generated_at,
        "registry_verified": True,
        "observers_total": len(observers),
        "observers_healthy": sum(item["healthy"] for item in observers),
        "observer_connections": sum(item["peer_links"] for item in observers),
        "observers": observers,
        "error": None,
    }
    atomic_json(args.observer_discovery, [discovery_entry(item) for item in registrations], mode=0o644)
    atomic_json(args.state, state)
    return state


def main() -> int:
    args = arguments()
    try:
        state = reconcile(args)
    except Exception as exc:
        print(f"observer registry reconciliation failed: {exc}")
        return 2
    if not state.get("configured"):
        print("observer registry not configured")
    else:
        print(
            "observer registry reconciled: "
            f"{state['observers_healthy']}/{state['observers_total']} healthy"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
