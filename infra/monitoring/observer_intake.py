#!/usr/bin/env python3
"""Replay-safe public observer admission and registry promotion service."""

from __future__ import annotations

from collections import defaultdict, deque
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from datetime import datetime, timezone
from http.client import HTTPResponse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import ssl
import subprocess
import tempfile
import threading
import time
from urllib.parse import urlsplit


SUBMISSION_SCHEMA = "power-house-observer-intake-submission-v1"
STATUS_SCHEMA = "power-house-observer-intake-status-v1"
INDEX_SCHEMA = "power-house-observer-intake-index-v1"
REGISTRATION_SCHEMA = "power-house-observer-registration-v1"
REGISTRY_SCHEMA = "power-house-observer-registry-v1"
MAX_BODY_BYTES = 64 * 1024
MAX_METRICS_BYTES = 2 * 1024 * 1024
IDENTITY_METRIC = re.compile(
    r'^powerhouse_node_identity\{(?P<labels>.+)\}\s+(?P<value>[0-9.eE+-]+)$'
)
SIMPLE_METRIC = re.compile(
    r"^(?P<name>[a-zA-Z_:][a-zA-Z0-9_:]*)\s+(?P<value>[0-9.eE+-]+)$"
)
LABEL = re.compile(r'([a-zA-Z_][a-zA-Z0-9_]*)="((?:\\.|[^"])*)"')
TRACKING_ID = re.compile(r"^obs_[a-f0-9]{32}$")


class IntakeError(RuntimeError):
    def __init__(self, code: str, message: str, status: int = 400, retryable: bool = False):
        super().__init__(message)
        self.code = code
        self.message = message
        self.status = status
        self.retryable = retryable


@dataclass(frozen=True)
class Settings:
    binary: Path = Path(os.environ.get("OBSERVER_INTAKE_BINARY", "/usr/local/bin/julian"))
    registry: Path = Path(
        os.environ.get("OBSERVER_INTAKE_REGISTRY", "/etc/powerhouse/observer-registry.json")
    )
    state_dir: Path = Path(
        os.environ.get(
            "OBSERVER_INTAKE_STATE_DIR", "/var/lib/powerhouse/observer-intake"
        )
    )
    chain_id: int = int(os.environ.get("OBSERVER_INTAKE_CHAIN_ID", "177155"))
    listen_host: str = os.environ.get("OBSERVER_INTAKE_HOST", "127.0.0.1")
    listen_port: int = int(os.environ.get("OBSERVER_INTAKE_PORT", "9195"))
    probe_timeout: float = float(os.environ.get("OBSERVER_INTAKE_PROBE_TIMEOUT", "6"))
    verifier_timeout: float = float(
        os.environ.get("OBSERVER_INTAKE_VERIFIER_TIMEOUT", "15")
    )
    auto_promote: bool = os.environ.get("OBSERVER_INTAKE_AUTO_PROMOTE", "1") == "1"
    allow_private_targets: bool = (
        os.environ.get("OBSERVER_INTAKE_ALLOW_PRIVATE_TARGETS", "0") == "1"
    )
    rate_limit: int = int(os.environ.get("OBSERVER_INTAKE_RATE_LIMIT", "12"))
    rate_window: int = int(os.environ.get("OBSERVER_INTAKE_RATE_WINDOW", "60"))
    queue_limit: int = int(os.environ.get("OBSERVER_INTAKE_QUEUE_LIMIT", "1000"))
    max_submissions: int = int(os.environ.get("OBSERVER_INTAKE_MAX_SUBMISSIONS", "10000"))
    retention_seconds: int = int(
        os.environ.get("OBSERVER_INTAKE_RETENTION_SECONDS", str(30 * 24 * 60 * 60))
    )
    allowed_origins: tuple[str, ...] = tuple(
        value.strip()
        for value in os.environ.get(
            "OBSERVER_INTAKE_ALLOWED_ORIGINS", "https://mfenx.com"
        ).split(",")
        if value.strip()
    )
    trusted_proxies: tuple[str, ...] = tuple(
        value.strip()
        for value in os.environ.get("OBSERVER_INTAKE_TRUSTED_PROXIES", "127.0.0.1,::1").split(",")
        if value.strip()
    )


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def canonical_json(value: object) -> bytes:
    return json.dumps(value, separators=(",", ":"), sort_keys=True).encode()


def digest_json(value: object) -> str:
    return hashlib.sha256(canonical_json(value)).hexdigest()


def audit(event: str, **fields: object) -> None:
    print(
        json.dumps(
            {"time": now_iso(), "event": event, **fields},
            separators=(",", ":"),
            sort_keys=True,
        ),
        flush=True,
    )


def atomic_bytes(path: Path, payload: bytes, mode: int = 0o640) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
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


def atomic_json(path: Path, value: object, mode: int = 0o640) -> None:
    atomic_bytes(path, json.dumps(value, indent=2, sort_keys=True).encode() + b"\n", mode)


def decode_label(value: str) -> str:
    return (
        value.replace(r"\\", "\0")
        .replace(r'\"', '"')
        .replace(r"\n", "\n")
        .replace("\0", "\\")
    )


def parse_metrics(body: str) -> tuple[dict[str, str] | None, dict[str, float]]:
    identity = None
    metrics: dict[str, float] = {}
    for line in body.splitlines():
        if not line or line.startswith("#"):
            continue
        match = IDENTITY_METRIC.fullmatch(line)
        if match:
            labels = {
                name: decode_label(value)
                for name, value in LABEL.findall(match.group("labels"))
            }
            if float(match.group("value")) == 1:
                identity = labels
            continue
        metric = SIMPLE_METRIC.fullmatch(line)
        if metric:
            metrics[metric.group("name")] = float(metric.group("value"))
    return identity, metrics


def resolve_addresses(host: str, allow_private: bool) -> list[ipaddress._BaseAddress]:
    if not host or "://" in host or "/" in host:
        raise IntakeError("invalid_endpoint", "endpoint host is invalid")
    try:
        addresses = [ipaddress.ip_address(host.strip("[]"))]
    except ValueError:
        try:
            records = socket.getaddrinfo(host, None, type=socket.SOCK_STREAM)
        except socket.gaierror as exc:
            raise IntakeError(
                "endpoint_unreachable", f"endpoint host did not resolve: {exc}", retryable=True
            ) from exc
        addresses = sorted(
            {ipaddress.ip_address(record[4][0]) for record in records}, key=str
        )
    if not addresses:
        raise IntakeError("endpoint_unreachable", "endpoint host did not resolve", retryable=True)
    if not allow_private:
        rejected = [address for address in addresses if not address.is_global]
        if rejected:
            raise IntakeError(
                "private_endpoint",
                f"endpoint resolves to non-public address {rejected[0]}",
            )
    return addresses


def parse_multiaddr(address: str) -> tuple[str, int, str]:
    parts = [part for part in address.split("/") if part]
    values = dict(zip(parts[::2], parts[1::2]))
    host = next((values[key] for key in ("ip4", "ip6", "dns", "dns4", "dns6") if key in values), None)
    if not host or "tcp" not in values or "p2p" not in values:
        raise IntakeError("invalid_endpoint", "p2p address must include host, tcp, and p2p")
    try:
        port = int(values["tcp"])
    except ValueError as exc:
        raise IntakeError("invalid_endpoint", "p2p TCP port is invalid") from exc
    if not 1 <= port <= 65535:
        raise IntakeError("invalid_endpoint", "p2p TCP port is invalid")
    return host, port, values["p2p"]


def open_socket(address: ipaddress._BaseAddress, port: int, timeout: float):
    family = socket.AF_INET6 if address.version == 6 else socket.AF_INET
    sock = socket.socket(family, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    try:
        sock.connect((str(address), port))
        return sock
    except Exception:
        sock.close()
        raise


def metrics_request(
    url: str, addresses: list[ipaddress._BaseAddress], timeout: float
) -> tuple[str, str]:
    parsed = urlsplit(url)
    if parsed.scheme not in {"http", "https"} or not parsed.hostname:
        raise IntakeError("invalid_endpoint", "metrics URL must use http or https")
    port = parsed.port or (443 if parsed.scheme == "https" else 80)
    host_header = parsed.hostname
    if ":" in host_header:
        host_header = f"[{host_header}]"
    if port != (443 if parsed.scheme == "https" else 80):
        host_header = f"{host_header}:{port}"
    failures = []
    for address in addresses:
        sock = None
        try:
            sock = open_socket(address, port, timeout)
            if parsed.scheme == "https":
                context = ssl.create_default_context()
                sock = context.wrap_socket(sock, server_hostname=parsed.hostname)
            request = (
                f"GET {parsed.path or '/'} HTTP/1.1\r\n"
                f"Host: {host_header}\r\n"
                "User-Agent: power-house-observer-intake/1\r\n"
                "Accept: text/plain\r\n"
                "Connection: close\r\n\r\n"
            ).encode()
            sock.sendall(request)
            response = HTTPResponse(sock)
            response.begin()
            if response.status != 200:
                raise RuntimeError(f"HTTP {response.status}")
            payload = response.read(MAX_METRICS_BYTES + 1)
            if len(payload) > MAX_METRICS_BYTES:
                raise RuntimeError("metrics response exceeds 2 MiB")
            return payload.decode("utf-8", "replace"), str(address)
        except Exception as exc:
            failures.append(f"{address}: {exc}")
        finally:
            if sock is not None:
                try:
                    sock.close()
                except OSError:
                    pass
    raise IntakeError(
        "endpoint_unreachable",
        "metrics endpoint unavailable: " + "; ".join(failures),
        retryable=True,
    )


def tcp_probe(
    addresses: list[ipaddress._BaseAddress], port: int, timeout: float
) -> str:
    failures = []
    for address in addresses:
        try:
            with open_socket(address, port, timeout):
                return str(address)
        except Exception as exc:
            failures.append(f"{address}: {exc}")
    raise IntakeError(
        "endpoint_unreachable",
        "p2p endpoint unavailable: " + "; ".join(failures),
        retryable=True,
    )


class ObserverIntake:
    def __init__(self, settings: Settings):
        self.settings = settings
        self.submissions = settings.state_dir / "submissions"
        self.history = settings.state_dir / "history"
        self.index_path = settings.state_dir / "index.json"
        self.lock = threading.RLock()
        self.executor = ThreadPoolExecutor(max_workers=4, thread_name_prefix="observer-intake")
        self.rate_hits: dict[str, deque[float]] = defaultdict(deque)
        self._prepare()

    def _prepare(self) -> None:
        self.submissions.mkdir(parents=True, exist_ok=True, mode=0o750)
        self.history.mkdir(parents=True, exist_ok=True, mode=0o750)
        if not self.index_path.exists():
            atomic_json(
                self.index_path,
                {
                    "schema": INDEX_SCHEMA,
                    "digests": {},
                    "active_peers": {},
                    "latest_peers": {},
                },
            )
        index = self._read_index()
        self._import_registry(index)
        pending = []
        for path in self.submissions.glob("obs_*.json"):
            state = json.loads(path.read_text(encoding="utf-8"))
            if state.get("status") in {"queued", "verifying"} or (
                self.settings.auto_promote and state.get("status") == "approved"
            ):
                tracking_id = state.get("tracking_id")
                if TRACKING_ID.fullmatch(str(tracking_id)):
                    index["active_peers"][state["registration"]["peer_id"]] = tracking_id
                    pending.append(tracking_id)
        self._write_index(index)
        for tracking_id in pending:
            self.executor.submit(self.process, tracking_id)

    def _import_registry(self, index: dict) -> None:
        if not self.settings.registry.exists():
            return
        self._verify_registry_file(self.settings.registry)
        registry = json.loads(self.settings.registry.read_text(encoding="utf-8"))
        revision = digest_json(registry)
        timestamp = now_iso()
        for registration in registry.get("registrations", []):
            digest = digest_json(registration)
            tracking_id = index["digests"].get(digest)
            if not tracking_id or not self._state_path(tracking_id).exists():
                tracking_id = f"obs_{secrets.token_hex(16)}"
                state = {
                    "schema": SUBMISSION_SCHEMA,
                    "tracking_id": tracking_id,
                    "status": "promoted",
                    "created_at": timestamp,
                    "updated_at": timestamp,
                    "attempts": 0,
                    "digest": digest,
                    "registration": registration,
                    "checks": {"registry_import": "verified"},
                    "registry_revision": revision,
                    "error": None,
                    "retryable": False,
                }
                self._write_state(state)
                index["digests"][digest] = tracking_id
            peer_id = registration["peer_id"]
            current = index["latest_peers"].get(peer_id)
            if current is None or registration["issued_at_unix"] > current["issued_at_unix"]:
                index["latest_peers"][peer_id] = {
                    "issued_at_unix": registration["issued_at_unix"],
                    "tracking_id": tracking_id,
                }

    def close(self) -> None:
        self.executor.shutdown(wait=True, cancel_futures=False)

    def _read_index(self) -> dict:
        try:
            value = json.loads(self.index_path.read_text(encoding="utf-8"))
        except (FileNotFoundError, json.JSONDecodeError) as exc:
            raise RuntimeError(f"observer intake index unavailable: {exc}") from exc
        if value.get("schema") != INDEX_SCHEMA:
            raise RuntimeError("observer intake index schema mismatch")
        for field in ("digests", "active_peers", "latest_peers"):
            if not isinstance(value.get(field), dict):
                raise RuntimeError(f"observer intake index {field} is invalid")
        return value

    def _write_index(self, value: dict) -> None:
        atomic_json(self.index_path, value)

    def _prune(self, index: dict) -> bool:
        cutoff = datetime.now(timezone.utc).timestamp() - self.settings.retention_seconds
        changed = False
        for path in self.submissions.glob("obs_*.json"):
            try:
                state = json.loads(path.read_text(encoding="utf-8"))
                updated = datetime.fromisoformat(state["updated_at"].replace("Z", "+00:00"))
            except (KeyError, ValueError, json.JSONDecodeError):
                continue
            if state.get("status") not in {"promoted", "rejected"}:
                continue
            if updated.timestamp() >= cutoff:
                continue
            path.unlink()
            changed = True
            digest = state.get("digest")
            tracking_id = state.get("tracking_id")
            if digest and index["digests"].get(digest) == tracking_id:
                index["digests"].pop(digest, None)
        return changed

    def _state_path(self, tracking_id: str) -> Path:
        if not TRACKING_ID.fullmatch(tracking_id):
            raise IntakeError("not_found", "registration status not found", 404)
        return self.submissions / f"{tracking_id}.json"

    def _read_state(self, tracking_id: str) -> dict:
        try:
            return json.loads(self._state_path(tracking_id).read_text(encoding="utf-8"))
        except FileNotFoundError as exc:
            raise IntakeError("not_found", "registration status not found", 404) from exc

    def _write_state(self, state: dict) -> None:
        state["updated_at"] = now_iso()
        atomic_json(self._state_path(state["tracking_id"]), state)

    @staticmethod
    def public_state(state: dict, duplicate: bool = False) -> dict:
        return {
            "schema": STATUS_SCHEMA,
            "tracking_id": state["tracking_id"],
            "status": state["status"],
            "created_at": state["created_at"],
            "updated_at": state["updated_at"],
            "node_id": state["registration"].get("node_id"),
            "peer_id": state["registration"].get("peer_id"),
            "checks": state.get("checks", {}),
            "attempts": state.get("attempts", 0),
            "registry_revision": state.get("registry_revision"),
            "error": state.get("error"),
            "retryable": state.get("retryable", False),
            "duplicate": duplicate,
        }

    def rate_limit(self, client: str) -> None:
        now = time.monotonic()
        with self.lock:
            hits = self.rate_hits[client]
            while hits and hits[0] <= now - self.settings.rate_window:
                hits.popleft()
            if len(hits) >= self.settings.rate_limit:
                raise IntakeError("rate_limited", "registration rate limit exceeded", 429, True)
            hits.append(now)

    def submit(self, payload: object, background: bool = True) -> tuple[dict, int]:
        if not isinstance(payload, dict):
            raise IntakeError("invalid_json", "request body must be a JSON object")
        registration = payload.get("registration", payload)
        if not isinstance(registration, dict):
            raise IntakeError("invalid_registration", "registration must be a JSON object")
        digest = digest_json(registration)
        peer_id = str(registration.get("peer_id", ""))
        issued_at = registration.get("issued_at_unix")
        with self.lock:
            index = self._read_index()
            if self._prune(index):
                self._write_index(index)
            if sum(1 for _ in self.submissions.glob("obs_*.json")) >= self.settings.max_submissions:
                raise IntakeError(
                    "storage_full",
                    "registration storage limit is reached",
                    503,
                    True,
                )
            previous = index["digests"].get(digest)
            if previous:
                audit("duplicate", tracking_id=previous, peer_id=peer_id)
                return self.public_state(self._read_state(previous), duplicate=True), 200
            active = index["active_peers"].get(peer_id)
            if active:
                raise IntakeError(
                    "identity_pending",
                    f"this peer already has active submission {active}",
                    409,
                    True,
                )
            latest = index["latest_peers"].get(peer_id)
            if latest and isinstance(issued_at, int) and issued_at <= latest["issued_at_unix"]:
                raise IntakeError(
                    "replayed_registration",
                    "registration issue time does not advance the admitted identity",
                    409,
                )
            active_count = sum(
                1
                for path in self.submissions.glob("obs_*.json")
                if json.loads(path.read_text(encoding="utf-8")).get("status")
                in {"queued", "verifying", "approved"}
            )
            if active_count >= self.settings.queue_limit:
                raise IntakeError("queue_full", "registration queue is full", 503, True)
            tracking_id = f"obs_{secrets.token_hex(16)}"
            timestamp = now_iso()
            state = {
                "schema": SUBMISSION_SCHEMA,
                "tracking_id": tracking_id,
                "status": "queued",
                "created_at": timestamp,
                "updated_at": timestamp,
                "attempts": 0,
                "digest": digest,
                "registration": registration,
                "checks": {},
                "registry_revision": None,
                "error": None,
                "retryable": False,
            }
            self._write_state(state)
            index["digests"][digest] = tracking_id
            if peer_id:
                index["active_peers"][peer_id] = tracking_id
            self._write_index(index)
            audit("submitted", tracking_id=tracking_id, peer_id=peer_id)
        if background:
            self.executor.submit(self.process, tracking_id)
        else:
            self.process(tracking_id)
        return self.public_state(self._read_state(tracking_id)), 202

    def retry(self, tracking_id: str, background: bool = True) -> tuple[dict, int]:
        with self.lock:
            state = self._read_state(tracking_id)
            if state.get("status") not in {"rejected", "approved"} or not state.get("retryable"):
                raise IntakeError("not_retryable", "registration is not retryable", 409)
            peer_id = state["registration"]["peer_id"]
            index = self._read_index()
            active = index["active_peers"].get(peer_id)
            if active and active != tracking_id:
                raise IntakeError("identity_pending", "another submission is active", 409, True)
            index["active_peers"][peer_id] = tracking_id
            self._write_index(index)
            state["status"] = "queued"
            state["error"] = None
            state["retryable"] = False
            self._write_state(state)
            audit("retry_queued", tracking_id=tracking_id, peer_id=peer_id)
        if background:
            self.executor.submit(self.process, tracking_id)
        else:
            self.process(tracking_id)
        return self.public_state(self._read_state(tracking_id)), 202

    def _verify_registry_file(self, path: Path) -> dict:
        process = subprocess.run(
            [
                str(self.settings.binary),
                "observer-registry",
                "verify",
                str(path),
                "--now",
                str(int(time.time())),
                "--json",
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=self.settings.verifier_timeout,
        )
        if process.returncode:
            detail = process.stderr.strip() or process.stdout.strip() or "verification failed"
            raise IntakeError("verification_failed", detail)
        try:
            verified = json.loads(process.stdout)
        except json.JSONDecodeError as exc:
            raise IntakeError("verification_failed", "verifier returned invalid JSON") from exc
        if verified.get("verified") is not True:
            raise IntakeError("verification_failed", "verifier did not confirm registration")
        return verified

    def _verify_registration(self, registration: dict) -> None:
        candidate = {
            "schema": REGISTRY_SCHEMA,
            "chain_id": self.settings.chain_id,
            "registrations": [registration],
        }
        descriptor, temporary = tempfile.mkstemp(
            prefix=".observer-intake-verify.", suffix=".json", dir=self.settings.state_dir
        )
        os.close(descriptor)
        path = Path(temporary)
        try:
            atomic_json(path, candidate)
            self._verify_registry_file(path)
        finally:
            path.unlink(missing_ok=True)

    def _probe_registration(self, registration: dict) -> dict:
        p2p_host, p2p_port, address_peer = parse_multiaddr(registration["p2p_address"])
        parsed = urlsplit(registration["metrics_url"])
        if not parsed.hostname or parsed.hostname.lower() != p2p_host.lower():
            raise IntakeError("identity_mismatch", "metrics and p2p hosts differ")
        if address_peer != registration["peer_id"]:
            raise IntakeError("identity_mismatch", "p2p address peer ID differs")
        addresses = resolve_addresses(p2p_host, self.settings.allow_private_targets)
        metrics_body, metrics_address = metrics_request(
            registration["metrics_url"], addresses, self.settings.probe_timeout
        )
        identity, metrics = parse_metrics(metrics_body)
        expected_identity = {
            "node_id": registration["node_id"],
            "peer_id": registration["peer_id"],
            "public_key_b64": registration["public_key_b64"],
            "chain_id": str(registration["chain_id"]),
        }
        if identity != expected_identity:
            raise IntakeError(
                "identity_mismatch",
                "live metrics identity does not exactly match the signed registration",
            )
        peers = metrics.get("powerhouse_connected_peers")
        if peers is None or peers < 0 or not peers.is_integer():
            raise IntakeError(
                "identity_mismatch", "connected peer metric is missing or invalid"
            )
        p2p_address = tcp_probe(addresses, p2p_port, self.settings.probe_timeout)
        return {
            "signature": "verified",
            "identity": "verified",
            "metrics": "reachable",
            "p2p": "reachable",
            "metrics_address": metrics_address,
            "p2p_address": p2p_address,
            "connected_peers": int(peers),
        }

    def _candidate_registry(self, registration: dict) -> dict:
        if self.settings.registry.exists():
            current = json.loads(self.settings.registry.read_text(encoding="utf-8"))
            if current.get("schema") != REGISTRY_SCHEMA:
                raise IntakeError("registry_update_failed", "current registry schema is invalid", 500, True)
            if current.get("chain_id") != self.settings.chain_id:
                raise IntakeError("registry_update_failed", "current registry chain ID differs", 500, True)
            registrations = list(current.get("registrations", []))
        else:
            registrations = []
        replaced = False
        for index, existing in enumerate(registrations):
            same_peer = existing.get("peer_id") == registration.get("peer_id")
            same_key = existing.get("public_key_b64") == registration.get("public_key_b64")
            same_node = existing.get("node_id") == registration.get("node_id")
            if same_peer or same_key or same_node:
                if not (same_peer and same_key and same_node):
                    raise IntakeError(
                        "identity_collision",
                        "node ID, peer ID, or public key collides with an admitted identity",
                        409,
                    )
                if registration["issued_at_unix"] <= existing["issued_at_unix"]:
                    raise IntakeError(
                        "replayed_registration",
                        "registration does not advance the admitted identity",
                        409,
                    )
                registrations[index] = registration
                replaced = True
                break
        if not replaced:
            registrations.append(registration)
        registrations.sort(key=lambda item: item["peer_id"])
        return {
            "schema": REGISTRY_SCHEMA,
            "chain_id": self.settings.chain_id,
            "registrations": registrations,
        }

    def _replace_registry(self, source: Path, destination: Path) -> None:
        os.replace(source, destination)

    def _promote(self, registration: dict, tracking_id: str) -> str:
        candidate = self._candidate_registry(registration)
        self.settings.registry.parent.mkdir(parents=True, exist_ok=True)
        descriptor, temporary = tempfile.mkstemp(
            prefix=f".{self.settings.registry.name}.", dir=self.settings.registry.parent
        )
        os.close(descriptor)
        candidate_path = Path(temporary)
        old_payload = self.settings.registry.read_bytes() if self.settings.registry.exists() else None
        revision = digest_json(candidate)
        try:
            atomic_json(candidate_path, candidate)
            self._verify_registry_file(candidate_path)
            if old_payload is not None:
                atomic_bytes(
                    self.history / f"{int(time.time())}-{tracking_id}.json",
                    old_payload,
                )
            try:
                self._replace_registry(candidate_path, self.settings.registry)
                os.chmod(self.settings.registry, 0o640)
                self._verify_registry_file(self.settings.registry)
            except Exception as exc:
                if old_payload is None:
                    self.settings.registry.unlink(missing_ok=True)
                else:
                    atomic_bytes(self.settings.registry, old_payload)
                raise IntakeError(
                    "registry_update_failed",
                    f"atomic registry promotion rolled back: {exc}",
                    500,
                    True,
                ) from exc
            return revision
        finally:
            candidate_path.unlink(missing_ok=True)

    def process(self, tracking_id: str) -> None:
        with self.lock:
            state = self._read_state(tracking_id)
            state["status"] = "verifying"
            state["attempts"] = int(state.get("attempts", 0)) + 1
            state["error"] = None
            state["retryable"] = False
            self._write_state(state)
        try:
            self._verify_registration(state["registration"])
            checks = self._probe_registration(state["registration"])
            with self.lock:
                state = self._read_state(tracking_id)
                state["checks"] = checks
                state["status"] = "approved"
                self._write_state(state)
            if self.settings.auto_promote:
                revision = self._promote(state["registration"], tracking_id)
                with self.lock:
                    state = self._read_state(tracking_id)
                    state["status"] = "promoted"
                    state["registry_revision"] = revision
                    self._write_state(state)
                    audit(
                        "promoted",
                        tracking_id=tracking_id,
                        peer_id=state["registration"]["peer_id"],
                        registry_revision=revision,
                    )
        except IntakeError as exc:
            with self.lock:
                state = self._read_state(tracking_id)
                state["status"] = "approved" if exc.code == "registry_update_failed" else "rejected"
                state["error"] = {"code": exc.code, "message": exc.message}
                state["retryable"] = exc.retryable
                self._write_state(state)
                audit(
                    "admission_failed",
                    tracking_id=tracking_id,
                    peer_id=state["registration"].get("peer_id"),
                    code=exc.code,
                    retryable=exc.retryable,
                )
        except Exception as exc:
            with self.lock:
                state = self._read_state(tracking_id)
                state["status"] = "rejected"
                state["error"] = {"code": "internal_error", "message": str(exc)}
                state["retryable"] = True
                self._write_state(state)
                audit(
                    "admission_failed",
                    tracking_id=tracking_id,
                    peer_id=state["registration"].get("peer_id"),
                    code="internal_error",
                    retryable=True,
                )
        finally:
            with self.lock:
                state = self._read_state(tracking_id)
                index = self._read_index()
                peer_id = state["registration"].get("peer_id", "")
                if index["active_peers"].get(peer_id) == tracking_id:
                    index["active_peers"].pop(peer_id, None)
                if state["status"] == "promoted":
                    index["latest_peers"][peer_id] = {
                        "issued_at_unix": state["registration"]["issued_at_unix"],
                        "tracking_id": tracking_id,
                    }
                self._write_index(index)

    def registry_payload(self) -> bytes:
        try:
            return self.settings.registry.read_bytes()
        except FileNotFoundError as exc:
            raise IntakeError("not_found", "observer registry is not configured", 404) from exc


class IntakeHandler(BaseHTTPRequestHandler):
    server_version = "PowerHouseObserverIntake/1"

    @property
    def intake(self) -> ObserverIntake:
        return self.server.intake

    def _client_id(self) -> str:
        direct = self.client_address[0]
        if direct in self.intake.settings.trusted_proxies:
            forwarded = self.headers.get("X-Forwarded-For", "").split(",", 1)[0].strip()
            if forwarded:
                try:
                    return str(ipaddress.ip_address(forwarded))
                except ValueError:
                    pass
        return direct

    def _origin(self) -> str | None:
        return self.headers.get("Origin")

    def _check_origin(self) -> None:
        origin = self._origin()
        if origin and origin not in self.intake.settings.allowed_origins:
            raise IntakeError("origin_rejected", "request origin is not allowed", 403)

    def _send_json(self, status: int, value: object) -> None:
        payload = json.dumps(value, sort_keys=True).encode()
        self.send_response(status)
        origin = self._origin()
        if origin in self.intake.settings.allowed_origins:
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("X-Content-Type-Options", "nosniff")
        self.end_headers()
        self.wfile.write(payload)

    def _error(self, error: IntakeError) -> None:
        self._send_json(
            error.status,
            {
                "schema": STATUS_SCHEMA,
                "status": "error",
                "error": {"code": error.code, "message": error.message},
                "retryable": error.retryable,
            },
        )

    def _internal_error(self) -> None:
        self._error(
            IntakeError(
                "internal_error",
                "observer intake could not process the request",
                500,
                True,
            )
        )

    def _read_json(self, allow_empty: bool = False) -> object:
        raw_length = self.headers.get("Content-Length")
        if raw_length is None:
            if allow_empty:
                return {}
            raise IntakeError("length_required", "Content-Length is required", 411)
        try:
            length = int(raw_length)
        except ValueError as exc:
            raise IntakeError("invalid_length", "Content-Length is invalid") from exc
        if length < 0 or length > MAX_BODY_BYTES:
            raise IntakeError("body_too_large", "request body exceeds 64 KiB", 413)
        if not allow_empty and length == 0:
            raise IntakeError("invalid_json", "request body is empty")
        content_type = self.headers.get("Content-Type", "").split(";", 1)[0].strip().lower()
        if length and content_type != "application/json":
            raise IntakeError("unsupported_media_type", "Content-Type must be application/json", 415)
        payload = self.rfile.read(length)
        if not payload:
            return {}
        try:
            return json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise IntakeError("invalid_json", "request body is not valid JSON") from exc

    def do_OPTIONS(self):
        try:
            self._check_origin()
            self.send_response(204)
            origin = self._origin()
            if origin:
                self.send_header("Access-Control-Allow-Origin", origin)
                self.send_header("Vary", "Origin")
            self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "Content-Type")
            self.send_header("Access-Control-Max-Age", "600")
            self.send_header("Content-Length", "0")
            self.end_headers()
        except IntakeError as exc:
            self._error(exc)
        except Exception:
            self._internal_error()

    def do_POST(self):
        try:
            self._check_origin()
            self.intake.rate_limit(self._client_id())
            if self.path == "/observer-registrations":
                status, code = self.intake.submit(self._read_json())
            elif self.path.startswith("/observer-registrations/") and self.path.endswith("/retry"):
                tracking_id = self.path.split("/")[2]
                self._read_json(allow_empty=True)
                status, code = self.intake.retry(tracking_id)
            else:
                raise IntakeError("not_found", "endpoint not found", 404)
            self._send_json(code, status)
        except IntakeError as exc:
            self._error(exc)
        except Exception:
            self._internal_error()

    def do_GET(self):
        try:
            self._check_origin()
            if self.path == "/observer-intake-healthz":
                self._send_json(200, {"status": "ok", "schema": STATUS_SCHEMA})
                return
            if self.path == "/observer-registry.json":
                payload = self.intake.registry_payload()
                self.send_response(200)
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)
                return
            if self.path.startswith("/observer-registrations/"):
                tracking_id = self.path.split("/")[2]
                self._send_json(200, self.intake.public_state(self.intake._read_state(tracking_id)))
                return
            raise IntakeError("not_found", "endpoint not found", 404)
        except IntakeError as exc:
            self._error(exc)
        except Exception:
            self._internal_error()

    def log_message(self, format, *args):
        return


class IntakeServer(ThreadingHTTPServer):
    def __init__(self, address, intake: ObserverIntake):
        self.intake = intake
        super().__init__(address, IntakeHandler)


def main() -> int:
    settings = Settings()
    intake = ObserverIntake(settings)
    server = IntakeServer((settings.listen_host, settings.listen_port), intake)
    try:
        server.serve_forever()
    finally:
        server.server_close()
        intake.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
