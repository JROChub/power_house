#!/usr/bin/env python3
"""External, tamper-evident Power House reliability campaign controller."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shlex
import signal
import subprocess
import tempfile
import time
from urllib.request import Request, urlopen


CAMPAIGN_SCHEMA = "power-house-reliability-campaign-v1"
EVENT_SCHEMA = "power-house-reliability-event-v1"
CONFIG_SCHEMA = "power-house-reliability-config-v1"
SAFE_NAME = re.compile(r"^[a-zA-Z0-9_.@:-]+$")
SAFE_SERVICE = re.compile(r"^[a-zA-Z0-9_.@-]+\.service$")
SAFE_PATH = re.compile(r"^/[a-zA-Z0-9_./-]+$")
DRILL_TYPES = {"validator_failover", "intake_recovery", "replica_recovery"}


def now_iso(timestamp: float | None = None) -> str:
    value = time.time() if timestamp is None else timestamp
    return datetime.fromtimestamp(value, timezone.utc).isoformat()


def canonical_json(value: object) -> bytes:
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode()


def digest_json(value: object) -> str:
    return hashlib.sha256(canonical_json(value)).hexdigest()


def atomic_json(path: Path, value: object, mode: int = 0o600) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(json.dumps(value, indent=2, sort_keys=True).encode() + b"\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, mode)
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def percentile(values: list[float], quantile: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, math.ceil(quantile * len(ordered)) - 1))
    return round(ordered[index], 3)


@dataclass(frozen=True)
class Node:
    name: str
    target: str
    service: str
    state_path: str
    observer_registry_path: str

    @classmethod
    def from_dict(cls, value: dict) -> "Node":
        node = cls(
            name=str(value["name"]),
            target=str(value["target"]),
            service=str(value["service"]),
            state_path=str(value["state_path"]),
            observer_registry_path=str(value["observer_registry_path"]),
        )
        if not SAFE_NAME.fullmatch(node.name) or not SAFE_NAME.fullmatch(node.target):
            raise ValueError("node name or SSH target contains unsafe characters")
        if not SAFE_SERVICE.fullmatch(node.service):
            raise ValueError(f"invalid service name for {node.name}")
        if not SAFE_PATH.fullmatch(node.state_path) or not SAFE_PATH.fullmatch(
            node.observer_registry_path
        ):
            raise ValueError(f"invalid state path for {node.name}")
        return node


@dataclass(frozen=True)
class Drill:
    drill_id: str
    kind: str
    offset_seconds: int

    @classmethod
    def from_dict(cls, value: dict) -> "Drill":
        drill = cls(
            drill_id=str(value["id"]),
            kind=str(value["kind"]),
            offset_seconds=int(value["offset_seconds"]),
        )
        if not SAFE_NAME.fullmatch(drill.drill_id):
            raise ValueError("drill ID contains unsafe characters")
        if drill.kind not in DRILL_TYPES:
            raise ValueError(f"unsupported drill type: {drill.kind}")
        if drill.offset_seconds < 0:
            raise ValueError("drill offset cannot be negative")
        return drill


@dataclass(frozen=True)
class Config:
    state_dir: Path
    duration_seconds: int
    sample_interval_seconds: int
    burst_interval_seconds: int
    burst_requests: int
    recovery_timeout_seconds: int
    expected_chain_id: int
    expected_release: str
    rpc_url: str
    status_url: str
    intake_url: str
    primary_node: str
    nodes: tuple[Node, ...]
    drills: tuple[Drill, ...]
    ssh_options: tuple[str, ...]
    publish_targets: tuple[str, ...]
    publish_path: str

    @classmethod
    def load(cls, path: Path) -> "Config":
        value = json.loads(path.read_text(encoding="utf-8"))
        if value.get("schema") != CONFIG_SCHEMA:
            raise ValueError("campaign configuration schema mismatch")
        config = cls(
            state_dir=Path(value["state_dir"]).expanduser(),
            duration_seconds=int(value.get("duration_seconds", 72 * 60 * 60)),
            sample_interval_seconds=int(value.get("sample_interval_seconds", 60)),
            burst_interval_seconds=int(value.get("burst_interval_seconds", 3600)),
            burst_requests=int(value.get("burst_requests", 30)),
            recovery_timeout_seconds=int(value.get("recovery_timeout_seconds", 90)),
            expected_chain_id=int(value.get("expected_chain_id", 177155)),
            expected_release=str(value["expected_release"]),
            rpc_url=str(value["rpc_url"]),
            status_url=str(value["status_url"]),
            intake_url=str(value["intake_url"]),
            primary_node=str(value["primary_node"]),
            nodes=tuple(Node.from_dict(item) for item in value["nodes"]),
            drills=tuple(Drill.from_dict(item) for item in value.get("drills", [])),
            ssh_options=tuple(str(item) for item in value.get("ssh_options", [])),
            publish_targets=tuple(str(item) for item in value["publish_targets"]),
            publish_path=str(value["publish_path"]),
        )
        config.validate()
        return config

    def validate(self) -> None:
        if len(self.nodes) != 3 or len({node.name for node in self.nodes}) != 3:
            raise ValueError("campaign requires exactly three unique validators")
        if self.primary_node not in {node.name for node in self.nodes}:
            raise ValueError("primary node is not present in validator targets")
        if not 60 <= self.duration_seconds <= 14 * 24 * 60 * 60:
            raise ValueError("campaign duration is outside the safe range")
        if not 5 <= self.sample_interval_seconds <= 900:
            raise ValueError("sample interval is outside the safe range")
        if self.burst_interval_seconds < self.sample_interval_seconds:
            raise ValueError("burst interval must be at least one sample interval")
        if not 3 <= self.burst_requests <= 300:
            raise ValueError("burst request count is outside the safe range")
        if not 15 <= self.recovery_timeout_seconds <= 300:
            raise ValueError("recovery timeout is outside the safe range")
        if not re.fullmatch(r"\d+\.\d+\.\d+", self.expected_release):
            raise ValueError("expected release is invalid")
        if not all(url.startswith("https://") for url in (self.rpc_url, self.status_url, self.intake_url)):
            raise ValueError("public campaign endpoints must use HTTPS")
        if not SAFE_PATH.fullmatch(self.publish_path):
            raise ValueError("publish path contains unsafe characters")
        if not self.publish_path.startswith("/var/lib/powerhouse/reliability/"):
            raise ValueError("publish path must remain inside reliability state")
        if not self.publish_targets or any(
            not SAFE_NAME.fullmatch(target) for target in self.publish_targets
        ):
            raise ValueError("publish targets are missing or invalid")
        if any(drill.offset_seconds >= self.duration_seconds for drill in self.drills):
            raise ValueError("drill is scheduled after campaign completion")

    def fingerprint(self) -> str:
        public = {
            "duration_seconds": self.duration_seconds,
            "sample_interval_seconds": self.sample_interval_seconds,
            "burst_interval_seconds": self.burst_interval_seconds,
            "burst_requests": self.burst_requests,
            "expected_chain_id": self.expected_chain_id,
            "expected_release": self.expected_release,
            "nodes": [node.__dict__ for node in self.nodes],
            "drills": [drill.__dict__ for drill in self.drills],
            "primary_node": self.primary_node,
            "publish_targets": self.publish_targets,
            "publish_path": self.publish_path,
            "rpc_url": self.rpc_url,
            "status_url": self.status_url,
            "intake_url": self.intake_url,
        }
        return digest_json(public)


class Campaign:
    def __init__(self, config: Config):
        self.config = config
        self.config.state_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        self.state_path = self.config.state_dir / "campaign-state.json"
        self.public_path = self.config.state_dir / "campaign-status.json"
        self.events_path = self.config.state_dir / "events.jsonl"
        self.report_path = self.config.state_dir / "final-report.json"
        self.manifest_path = self.config.state_dir / "SHA256SUMS"
        self.lock_path = self.config.state_dir / "campaign.lock"
        self.stopping = False
        self.state = self._load_or_create()

    def _load_or_create(self) -> dict:
        if self.state_path.exists():
            state = json.loads(self.state_path.read_text(encoding="utf-8"))
            if state.get("schema") != CAMPAIGN_SCHEMA:
                raise RuntimeError("campaign state schema mismatch")
            if state.get("config_fingerprint") != self.config.fingerprint():
                raise RuntimeError("campaign configuration changed after start")
            interrupted = False
            for drill in state.get("drills", []):
                if drill.get("status") == "running":
                    drill["status"] = "failed"
                    drill["completed_at"] = now_iso()
                    drill["detail"] = "campaign controller restarted during drill"
                    interrupted = True
            if interrupted:
                state["phase"] = "soak"
            return state
        started = time.time()
        campaign_id = datetime.fromtimestamp(started, timezone.utc).strftime("rel_%Y%m%dT%H%M%SZ")
        state = {
            "schema": CAMPAIGN_SCHEMA,
            "campaign_id": campaign_id,
            "config_fingerprint": self.config.fingerprint(),
            "status": "running",
            "phase": "baseline",
            "started_unix": started,
            "ends_unix": started + self.config.duration_seconds,
            "updated_unix": started,
            "sample_count": 0,
            "successful_samples": 0,
            "failed_samples": 0,
            "consecutive_failures": 0,
            "max_consecutive_failures": 0,
            "rpc_requests": 0,
            "rpc_errors": 0,
            "rpc_latencies_ms": [],
            "last_sample": None,
            "last_sample_unix": None,
            "baseline": None,
            "drills": [
                {
                    "id": drill.drill_id,
                    "kind": drill.kind,
                    "offset_seconds": drill.offset_seconds,
                    "status": "scheduled",
                    "started_at": None,
                    "completed_at": None,
                    "recovery_seconds": None,
                    "errors_observed": None,
                    "detail": None,
                }
                for drill in self.config.drills
            ],
            "evidence_sequence": 0,
            "evidence_head": "0" * 64,
            "last_burst_unix": 0,
            "last_publish_error": None,
            "final_report_sha256": None,
        }
        atomic_json(self.state_path, state)
        return state

    @property
    def primary(self) -> Node:
        return next(node for node in self.config.nodes if node.name == self.config.primary_node)

    def _run(self, argv: list[str], timeout: float = 30) -> subprocess.CompletedProcess:
        try:
            return subprocess.run(
                argv,
                check=False,
                capture_output=True,
                text=True,
                timeout=timeout,
            )
        except subprocess.TimeoutExpired as exc:
            return subprocess.CompletedProcess(
                argv,
                124,
                stdout=exc.stdout or "",
                stderr=exc.stderr or f"command timed out after {timeout} seconds",
            )

    def _ssh(self, node: Node, command: str, timeout: float = 30) -> str:
        process = self._run(
            ["ssh", *self.config.ssh_options, node.target, command], timeout=timeout
        )
        if process.returncode:
            detail = process.stderr.strip() or process.stdout.strip() or "SSH command failed"
            raise RuntimeError(f"{node.name}: {detail}")
        return process.stdout

    def _http_json(
        self, url: str, *, data: bytes | None = None, timeout: float = 8
    ) -> tuple[dict, float]:
        request = Request(
            url,
            data=data,
            headers={"Content-Type": "application/json"} if data is not None else {},
        )
        started = time.monotonic()
        with urlopen(request, timeout=timeout) as response:
            value = json.loads(response.read())
            if response.status != 200:
                raise RuntimeError(f"HTTP {response.status}")
        return value, (time.monotonic() - started) * 1000

    def _rpc(self, method: str) -> tuple[object, float]:
        payload = json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": method, "params": []}
        ).encode()
        value, latency = self._http_json(self.config.rpc_url, data=payload)
        if "result" not in value:
            raise RuntimeError(f"{method} returned no result")
        return value["result"], latency

    def audit_node(self, node: Node) -> dict:
        registry = "/etc/powerhouse/validator-registry.json"
        command = "; ".join(
            [
                "set -u",
                "printf 'version='; /usr/local/bin/julian --version",
                f"printf 'binary='; sha256sum /usr/local/bin/julian | cut -d' ' -f1",
                f"printf 'registry='; sha256sum {shlex.quote(registry)} | cut -d' ' -f1",
                f"printf 'state='; sha256sum {shlex.quote(node.state_path)} | cut -d' ' -f1",
                f"printf 'observer='; jq -S . {shlex.quote(node.observer_registry_path)} | sha256sum | cut -d' ' -f1",
                f"printf 'service='; systemctl is-active {shlex.quote(node.service)} || true",
                "printf 'health='; curl -fsS http://127.0.0.1:8545/healthz; printf '\\n'",
                (
                    "printf 'alerts='; curl -fsS http://127.0.0.1:9090/api/v1/alerts; printf '\\n'"
                    if node.name == self.primary.name
                    else "printf 'alerts={}\\n'"
                ),
            ]
        )
        output = self._ssh(node, command)
        values = {}
        for line in output.splitlines():
            key, separator, value = line.partition("=")
            if separator:
                values[key] = value
        health = json.loads(values.get("health", "{}"))
        alert_payload = json.loads(values.get("alerts", "{}"))
        alerts = [
            item.get("labels", {}).get("alertname", "unknown")
            for item in alert_payload.get("data", {}).get("alerts", [])
            if item.get("state") == "firing"
        ]
        return {
            "name": node.name,
            "version": values.get("version", "").removeprefix("julian "),
            "binary_sha256": values.get("binary"),
            "validator_registry_sha256": values.get("registry"),
            "state_sha256": values.get("state"),
            "observer_registry_sha256": values.get("observer"),
            "service": values.get("service"),
            "health": health,
            "active_alerts": alerts,
        }

    def collect_sample(self) -> dict:
        errors = []
        latencies = []
        nodes = []
        try:
            status, latency = self._http_json(self.config.status_url)
            latencies.append(latency)
        except Exception as exc:
            status = {}
            errors.append(f"public status: {exc}")
        try:
            intake, latency = self._http_json(self.config.intake_url)
            latencies.append(latency)
        except Exception as exc:
            intake = {}
            errors.append(f"observer intake: {exc}")
        rpc_results = {}
        for method in ("eth_chainId", "eth_blockNumber", "web3_clientVersion"):
            try:
                result, latency = self._rpc(method)
                rpc_results[method] = result
                latencies.append(latency)
                self.state["rpc_requests"] += 1
            except Exception as exc:
                errors.append(f"RPC {method}: {exc}")
                self.state["rpc_requests"] += 1
                self.state["rpc_errors"] += 1
        for node in self.config.nodes:
            try:
                nodes.append(self.audit_node(node))
            except Exception as exc:
                errors.append(str(exc))

        if status:
            if status.get("status") != "operational":
                errors.append(f"network status is {status.get('status')}")
            if status.get("validators_healthy") != 3 or status.get("validators_total") != 3:
                errors.append("validator health is not 3/3")
            if status.get("release") != self.config.expected_release:
                errors.append("public release differs from campaign release")
            if status.get("validator_registry", {}).get("verified") is not True:
                errors.append("public validator registry is not verified")
            if status.get("observer_registry", {}).get("verified") is not True:
                errors.append("public observer registry is not verified")
        if intake.get("status") != "ok":
            errors.append("observer intake is not healthy")
        expected_chain = hex(self.config.expected_chain_id)
        if rpc_results.get("eth_chainId") != expected_chain:
            errors.append("RPC chain ID differs from campaign chain")
        if self.config.expected_release not in str(rpc_results.get("web3_clientVersion", "")):
            errors.append("RPC client release differs from campaign release")
        if len(nodes) == 3:
            for field in (
                "version",
                "binary_sha256",
                "validator_registry_sha256",
                "observer_registry_sha256",
            ):
                if len({node.get(field) for node in nodes}) != 1:
                    errors.append(f"validator {field} values differ")
            finalized = {
                (
                    node.get("health", {}).get("finalized_block"),
                    node.get("health", {}).get("finalized_hash"),
                )
                for node in nodes
            }
            if len(finalized) != 1:
                errors.append("finalized state differs across validators")
            if any(node.get("service") != "active" for node in nodes):
                errors.append("one or more validator services are inactive")
            if any(node.get("version") != self.config.expected_release for node in nodes):
                errors.append("one or more validator binaries have the wrong release")
            active_alerts = sorted(
                {
                    alert
                    for node in nodes
                    for alert in node.get("active_alerts", [])
                }
            )
            if active_alerts:
                errors.append("active Prometheus alerts: " + ", ".join(active_alerts))

        self.state["rpc_latencies_ms"].extend(latencies)
        self.state["rpc_latencies_ms"] = self.state["rpc_latencies_ms"][-10_000:]
        sample = {
            "recorded_at": now_iso(),
            "ok": not errors,
            "errors": errors,
            "latency_ms": {
                "last": round(latencies[-1], 3) if latencies else None,
                "sample_max": round(max(latencies), 3) if latencies else None,
            },
            "network": {
                "status": status.get("status"),
                "release": status.get("release"),
                "validators_healthy": status.get("validators_healthy"),
                "validators_total": status.get("validators_total"),
                "observers_healthy": status.get("observer_peers", {}).get("healthy"),
                "observers_total": status.get("observer_peers", {}).get("total"),
                "observer_connections": status.get("observer_peers", {}).get("connected"),
                "block_height": status.get("block_height"),
                "active_alerts": sum(
                    len(node.get("active_alerts", [])) for node in nodes
                ),
            },
            "rpc": rpc_results,
            "nodes": nodes,
        }
        return sample

    def record_event(self, kind: str, data: dict) -> str:
        sequence = int(self.state.get("evidence_sequence", 0)) + 1
        event = {
            "schema": EVENT_SCHEMA,
            "sequence": sequence,
            "recorded_at": now_iso(),
            "kind": kind,
            "previous_hash": self.state.get("evidence_head", "0" * 64),
            "data": data,
        }
        event_hash = digest_json(event)
        event["event_hash"] = event_hash
        with self.events_path.open("ab") as handle:
            handle.write(canonical_json(event) + b"\n")
            handle.flush()
            os.fsync(handle.fileno())
        self.state["evidence_sequence"] = sequence
        self.state["evidence_head"] = event_hash
        return event_hash

    def apply_sample(self, sample: dict, kind: str = "sample") -> None:
        sample_time = time.time()
        previous_time = self.state.get("last_sample_unix")
        if previous_time is not None:
            gap = sample_time - float(previous_time)
            if gap > self.config.sample_interval_seconds * 2:
                missed = max(1, int(gap // self.config.sample_interval_seconds) - 1)
                self.state["sample_count"] += missed
                self.state["failed_samples"] += missed
                self.state["consecutive_failures"] += missed
                self.state["max_consecutive_failures"] = max(
                    self.state["max_consecutive_failures"],
                    self.state["consecutive_failures"],
                )
                self.record_event(
                    "telemetry_gap",
                    {"gap_seconds": round(gap, 3), "missed_samples": missed},
                )
        self.state["sample_count"] += 1
        self.state["last_sample"] = sample
        self.state["last_sample_unix"] = sample_time
        if sample["ok"]:
            self.state["successful_samples"] += 1
            self.state["consecutive_failures"] = 0
        else:
            self.state["failed_samples"] += 1
            self.state["consecutive_failures"] += 1
            self.state["max_consecutive_failures"] = max(
                self.state["max_consecutive_failures"],
                self.state["consecutive_failures"],
            )
        if self.state.get("baseline") is None and sample["ok"]:
            self.state["baseline"] = {
                "recorded_at": sample["recorded_at"],
                "nodes": sample["nodes"],
                "network": sample["network"],
            }
            self.state["phase"] = "soak"
        self.record_event(kind, sample)

    def public_status(self) -> dict:
        now = time.time()
        started = float(self.state["started_unix"])
        ends = float(self.state["ends_unix"])
        elapsed = max(0, min(self.config.duration_seconds, int(now - started)))
        remaining = max(0, int(ends - now))
        samples = int(self.state["sample_count"])
        successful = int(self.state["successful_samples"])
        uptime = round(successful / samples * 100, 5) if samples else None
        drills = self.state.get("drills", [])
        completed = sum(1 for drill in drills if drill.get("status") == "passed")
        failed = sum(1 for drill in drills if drill.get("status") in {"failed", "blocked"})
        latencies = self.state.get("rpc_latencies_ms", [])
        last_sample = self.state.get("last_sample") or {}
        return {
            "schema": CAMPAIGN_SCHEMA,
            "campaign_id": self.state["campaign_id"],
            "status": self.state["status"],
            "phase": self.state["phase"],
            "started_at": now_iso(started),
            "ends_at": now_iso(ends),
            "updated_at": now_iso(),
            "duration_seconds": self.config.duration_seconds,
            "elapsed_seconds": elapsed,
            "remaining_seconds": remaining,
            "progress_percent": round(elapsed / self.config.duration_seconds * 100, 4),
            "sample_count": samples,
            "successful_samples": successful,
            "failed_samples": int(self.state["failed_samples"]),
            "max_consecutive_failures": int(self.state["max_consecutive_failures"]),
            "uptime_percent": uptime,
            "rpc": {
                "requests": int(self.state["rpc_requests"]),
                "errors": int(self.state["rpc_errors"]),
                "p50_ms": percentile(latencies, 0.50),
                "p95_ms": percentile(latencies, 0.95),
                "p99_ms": percentile(latencies, 0.99),
            },
            "network": last_sample.get("network", {}),
            "drills": {
                "scheduled": len(drills),
                "completed": completed,
                "failed": failed,
                "items": drills,
            },
            "evidence": {
                "events": int(self.state["evidence_sequence"]),
                "head_sha256": self.state["evidence_head"],
                "final_report_sha256": self.state.get("final_report_sha256"),
            },
        }

    def save(self, publish: bool = True) -> None:
        self.state["updated_unix"] = time.time()
        atomic_json(self.state_path, self.state)
        atomic_json(self.public_path, self.public_status(), mode=0o644)
        if publish:
            self.publish()

    def publish(self) -> None:
        destination_dir = str(Path(self.config.publish_path).parent)
        errors = []
        for target in self.config.publish_targets:
            temporary = f"/tmp/{self.state['campaign_id']}.status.json"
            upload = self._run(
                [
                    "scp",
                    *self.config.ssh_options,
                    str(self.public_path),
                    f"{target}:{temporary}",
                ],
                timeout=30,
            )
            if upload.returncode:
                errors.append(f"{target}: status upload failed")
                continue
            install = self._run(
                [
                    "ssh",
                    *self.config.ssh_options,
                    target,
                    " && ".join(
                        [
                            f"install -d -m 0755 {shlex.quote(destination_dir)}",
                            f"install -m 0644 {shlex.quote(temporary)} {shlex.quote(self.config.publish_path)}",
                            f"rm -f {shlex.quote(temporary)}",
                        ]
                    ),
                ],
                timeout=30,
            )
            if install.returncode:
                errors.append(f"{target}: status install failed")
        self.state["last_publish_error"] = "; ".join(errors) if errors else None
        atomic_json(self.state_path, self.state)

    def run_burst(self) -> dict:
        latencies = []
        errors = []
        methods = ("eth_chainId", "eth_blockNumber", "web3_clientVersion")
        for index in range(self.config.burst_requests):
            try:
                _, latency = self._rpc(methods[index % len(methods)])
                latencies.append(latency)
                self.state["rpc_requests"] += 1
            except Exception as exc:
                errors.append(str(exc))
                self.state["rpc_requests"] += 1
                self.state["rpc_errors"] += 1
        self.state["rpc_latencies_ms"].extend(latencies)
        self.state["rpc_latencies_ms"] = self.state["rpc_latencies_ms"][-10_000:]
        result = {
            "requests": self.config.burst_requests,
            "errors": len(errors),
            "error_detail": errors[:5],
            "p95_ms": percentile(latencies, 0.95),
            "max_ms": round(max(latencies), 3) if latencies else None,
        }
        self.record_event("rpc_burst", result)
        self.state["last_burst_unix"] = time.time()
        return result

    def _probe_during_recovery(self, service_node: Node, service: str) -> dict:
        deadline = time.monotonic() + self.config.recovery_timeout_seconds
        started = time.monotonic()
        requests = 0
        errors = 0
        max_consecutive = 0
        consecutive = 0
        service_active = False
        while time.monotonic() < deadline:
            requests += 1
            try:
                chain, latency = self._rpc("eth_chainId")
                self.state["rpc_requests"] += 1
                self.state["rpc_latencies_ms"].append(latency)
                if chain != hex(self.config.expected_chain_id):
                    raise RuntimeError("chain ID mismatch")
                consecutive = 0
            except Exception:
                errors += 1
                self.state["rpc_requests"] += 1
                self.state["rpc_errors"] += 1
                consecutive += 1
                max_consecutive = max(max_consecutive, consecutive)
            check = self._run(
                [
                    "ssh",
                    *self.config.ssh_options,
                    service_node.target,
                    f"systemctl is-active --quiet {shlex.quote(service)}",
                ],
                timeout=10,
            )
            if check.returncode == 0 and time.monotonic() - started >= 2:
                service_active = True
                break
            time.sleep(0.25)
        return {
            "recovery_seconds": round(time.monotonic() - started, 3),
            "requests": requests,
            "errors": errors,
            "max_consecutive_errors": max_consecutive,
            "service_active": service_active,
        }

    def _drill_action(self, kind: str) -> dict:
        if kind == "validator_failover":
            command = (
                f"systemctl kill --kill-who=main --signal=SIGKILL {shlex.quote(self.primary.service)}"
            )
            self._ssh(self.primary, command)
            result = self._probe_during_recovery(self.primary, self.primary.service)
            result["passed"] = result["service_active"] and result["errors"] == 0
            return result
        if kind == "intake_recovery":
            service = "powerhouse-observer-intake.service"
            self._ssh(
                self.primary,
                f"systemctl kill --kill-who=main --signal=SIGKILL {service}",
            )
            deadline = time.monotonic() + self.config.recovery_timeout_seconds
            started = time.monotonic()
            errors = 0
            requests = 0
            healthy = False
            while time.monotonic() < deadline:
                requests += 1
                try:
                    value, _ = self._http_json(self.config.intake_url)
                    if value.get("status") == "ok" and time.monotonic() - started >= 2:
                        healthy = True
                        break
                except Exception:
                    errors += 1
                time.sleep(0.25)
            return {
                "passed": healthy,
                "recovery_seconds": round(time.monotonic() - started, 3),
                "requests": requests,
                "errors": errors,
                "service_active": healthy,
            }
        replica = next(node for node in self.config.nodes if node.name != self.primary.name)
        path = shlex.quote(replica.observer_registry_path)
        backup = shlex.quote(f"{replica.observer_registry_path}.campaign-backup")
        command = "; ".join(
            [
                "set -eu",
                f"cp {path} {backup}",
                f"rm -f {path}",
                "if systemctl start powerhouse-observer-registry.service && "
                f"/usr/local/bin/julian observer-registry verify {path} --json >/dev/null; "
                f"then rm -f {backup}; else mv {backup} {path}; exit 1; fi",
            ]
        )
        started = time.monotonic()
        try:
            self._ssh(replica, command, timeout=self.config.recovery_timeout_seconds)
            return {
                "passed": True,
                "recovery_seconds": round(time.monotonic() - started, 3),
                "requests": 0,
                "errors": 0,
                "service_active": True,
            }
        except Exception as exc:
            return {
                "passed": False,
                "recovery_seconds": round(time.monotonic() - started, 3),
                "requests": 0,
                "errors": 1,
                "service_active": False,
                "detail": str(exc),
            }

    def perform_drill(self, drill: dict) -> dict:
        if drill.get("status") != "scheduled":
            return drill
        self.state["phase"] = f"drill:{drill['kind']}"
        drill["status"] = "running"
        drill["started_at"] = now_iso()
        self.save()
        preflight = self.collect_sample()
        self.apply_sample(preflight, "drill_preflight")
        if not preflight["ok"]:
            drill["status"] = "blocked"
            drill["completed_at"] = now_iso()
            drill["detail"] = "; ".join(preflight["errors"])
            self.record_event("drill_blocked", dict(drill))
            self.state["phase"] = "soak"
            self.save()
            return drill
        result = self._drill_action(drill["kind"])
        recovery = self.collect_sample()
        self.apply_sample(recovery, "drill_recovery")
        passed = result.get("passed") is True and recovery["ok"]
        drill["status"] = "passed" if passed else "failed"
        drill["completed_at"] = now_iso()
        drill["recovery_seconds"] = result.get("recovery_seconds")
        drill["errors_observed"] = result.get("errors")
        drill["detail"] = result.get("detail")
        self.record_event("drill_completed", {"drill": dict(drill), "result": result})
        self.state["phase"] = "soak"
        self.save()
        return drill

    def add_manual_drill(self, kind: str) -> dict:
        if kind not in DRILL_TYPES:
            raise ValueError(f"unsupported drill type: {kind}")
        drill = {
            "id": f"manual-{kind}-{int(time.time())}",
            "kind": kind,
            "offset_seconds": int(time.time() - self.state["started_unix"]),
            "status": "scheduled",
            "started_at": None,
            "completed_at": None,
            "recovery_seconds": None,
            "errors_observed": None,
            "detail": None,
        }
        self.state["drills"].append(drill)
        self.save()
        return self.perform_drill(drill)

    def finalize(self) -> None:
        scheduled_incomplete = any(
            drill["status"] not in {"passed"} for drill in self.state["drills"]
        )
        passed = self.state["failed_samples"] == 0 and not scheduled_incomplete
        self.state["status"] = "passed" if passed else "failed"
        self.state["phase"] = "complete"
        self.record_event(
            "campaign_completed",
            {
                "status": self.state["status"],
                "samples": self.state["sample_count"],
                "failed_samples": self.state["failed_samples"],
                "drills": self.state["drills"],
            },
        )
        report = self.public_status()
        report["evidence"]["events_sha256"] = sha256_file(self.events_path)
        atomic_json(self.report_path, report, mode=0o644)
        self.state["final_report_sha256"] = sha256_file(self.report_path)
        self.save(publish=False)
        manifest = [
            f"{sha256_file(self.events_path)}  {self.events_path.name}",
            f"{sha256_file(self.report_path)}  {self.report_path.name}",
            f"{sha256_file(self.public_path)}  {self.public_path.name}",
        ]
        self.manifest_path.write_text("\n".join(manifest) + "\n", encoding="ascii")
        os.chmod(self.manifest_path, 0o644)
        self.publish()

    def run_campaign(self) -> int:
        with self.lock_path.open("w") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise RuntimeError("another campaign controller is already running")
            if self.state["status"] in {"passed", "failed"}:
                self.save()
                return 0
            self.record_event("campaign_started", {"config": self.config.fingerprint()})
            self.save()
            if time.time() >= self.state["ends_unix"]:
                sample = self.collect_sample()
                self.apply_sample(sample, "final_sample")
                self.finalize()
                return 0
            next_sample = time.monotonic()
            while not self.stopping and time.time() < self.state["ends_unix"]:
                if time.monotonic() >= next_sample:
                    sample = self.collect_sample()
                    self.apply_sample(sample)
                    elapsed = time.time() - self.state["started_unix"]
                    for drill in self.state["drills"]:
                        if drill["status"] == "scheduled" and elapsed >= drill["offset_seconds"]:
                            self.perform_drill(drill)
                    if (
                        time.time() - self.state["last_burst_unix"]
                        >= self.config.burst_interval_seconds
                    ):
                        self.run_burst()
                    self.save()
                    next_sample = time.monotonic() + self.config.sample_interval_seconds
                time.sleep(min(1, max(0.05, next_sample - time.monotonic())))
            if not self.stopping and time.time() >= self.state["ends_unix"]:
                self.finalize()
            return 0


def parser() -> argparse.ArgumentParser:
    cli = argparse.ArgumentParser()
    sub = cli.add_subparsers(dest="command", required=True)
    for command in ("run", "sample", "status"):
        item = sub.add_parser(command)
        item.add_argument("--config", type=Path, required=True)
    drill = sub.add_parser("drill")
    drill.add_argument("kind", choices=sorted(DRILL_TYPES))
    drill.add_argument("--config", type=Path, required=True)
    return cli


def main() -> int:
    args = parser().parse_args()
    campaign = Campaign(Config.load(args.config))

    def stop(_signal, _frame):
        campaign.stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    if args.command == "run":
        return campaign.run_campaign()
    if args.command == "status":
        print(json.dumps(campaign.public_status(), indent=2, sort_keys=True))
        return 0
    if args.command == "sample":
        sample = campaign.collect_sample()
        campaign.apply_sample(sample)
        campaign.save()
        print(json.dumps(campaign.public_status(), indent=2, sort_keys=True))
        return 0 if sample["ok"] else 1
    result = campaign.add_manual_drill(args.kind)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
