#!/usr/bin/env python3

from __future__ import annotations

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target" / "debug" / "julian"
sys.path.insert(0, str(ROOT / "infra" / "monitoring"))
import observer_intake as intake_module


class MetricsServer:
    def __init__(self) -> None:
        self.body = ""
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path != "/metrics":
                    self.send_error(404)
                    return
                payload = owner.body.encode()
                self.send_response(200)
                self.send_header("Content-Type", "text/plain")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            def log_message(self, format, *args):
                return

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.server.server_port}/metrics"

    def close(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)


class P2PServer:
    def __init__(self) -> None:
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.socket.bind(("127.0.0.1", 0))
        self.socket.listen()
        self.socket.settimeout(0.2)
        self.port = self.socket.getsockname()[1]
        self.running = True
        self.thread = threading.Thread(target=self._serve, daemon=True)
        self.thread.start()

    def _serve(self) -> None:
        while self.running:
            try:
                connection, _ = self.socket.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            connection.close()

    def close(self) -> None:
        self.running = False
        self.socket.close()
        self.thread.join(timeout=2)


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(BINARY), *args],
        check=check,
        capture_output=True,
        text=True,
    )


def create_registration(
    base: Path,
    name: str,
    key_byte: int,
    metrics_url: str,
    p2p_port: int,
    issued_at: int,
    valid_until: int,
    operator: str = "External Observer",
) -> tuple[dict, dict]:
    key = base / f"{name}.key"
    key.write_bytes(bytes([key_byte]) * 32)
    identity = json.loads(run("key-info", str(key), "--json").stdout)
    output = base / f"{name}.registration.json"
    run(
        "observer-registry",
        "create",
        "--key",
        str(key),
        "--node-id",
        name,
        "--operator",
        operator,
        "--region",
        "external-test",
        "--p2p-address",
        f"/ip4/127.0.0.1/tcp/{p2p_port}/p2p/{identity['peer_id']}",
        "--metrics-url",
        metrics_url,
        "--issued-at",
        str(issued_at),
        "--valid-until",
        str(valid_until),
        "--output",
        str(output),
    )
    return json.loads(output.read_text()), identity


def metrics_body(name: str, identity: dict, peers: int = 3) -> str:
    return (
        "# TYPE powerhouse_node_identity gauge\n"
        "powerhouse_node_identity{"
        f'node_id="{name}",'
        f'peer_id="{identity["peer_id"]}",'
        f'public_key_b64="{identity["public_key_b64"]}",'
        'chain_id="177155"} 1\n'
        "# TYPE powerhouse_connected_peers gauge\n"
        f"powerhouse_connected_peers {peers}\n"
    )


def settings(base: Path, rate_limit: int = 50) -> intake_module.Settings:
    return intake_module.Settings(
        binary=BINARY,
        registry=base / "observer-registry.json",
        state_dir=base / "intake",
        chain_id=177155,
        listen_host="127.0.0.1",
        listen_port=0,
        probe_timeout=1,
        verifier_timeout=10,
        auto_promote=True,
        allow_private_targets=True,
        rate_limit=rate_limit,
        rate_window=60,
        queue_limit=100,
        allowed_origins=("https://mfenx.com",),
        trusted_proxies=("127.0.0.1",),
    )


def http_json(url: str, method: str = "GET", value: object | None = None, origin: str | None = None):
    payload = None if value is None else json.dumps(value).encode()
    headers = {}
    if payload is not None:
        headers["Content-Type"] = "application/json"
    if origin:
        headers["Origin"] = origin
    request = Request(url, data=payload, method=method, headers=headers)
    try:
        with urlopen(request, timeout=5) as response:
            return response.status, json.loads(response.read())
    except HTTPError as error:
        return error.code, json.loads(error.read())


def wait_for_status(url: str, terminal=("promoted", "rejected")) -> dict:
    deadline = time.time() + 8
    while time.time() < deadline:
        status, value = http_json(url)
        assert status == 200
        if value["status"] in terminal:
            return value
        time.sleep(0.05)
    raise AssertionError("intake status did not become terminal")


def assert_rejected(service: intake_module.ObserverIntake, registration: dict, code: str) -> None:
    status, response_code = service.submit(registration, background=False)
    assert response_code == 202
    assert status["status"] == "rejected"
    assert status["error"]["code"] == code


def main() -> None:
    registration_html = (ROOT / "publicpower" / "register.html").read_text()
    registration_js = (ROOT / "publicpower" / "register.js").read_text()
    assert 'id="submit-registration"' in registration_html
    assert 'id="admission-tracking"' in registration_html
    assert "/observer-registrations" in registration_js
    assert "github-submit" not in registration_html
    if not BINARY.exists():
        subprocess.run(
            ["cargo", "build", "--features", "net", "--bin", "julian"],
            cwd=ROOT,
            check=True,
        )
    metrics_one = MetricsServer()
    metrics_two = MetricsServer()
    p2p = P2PServer()
    try:
        with tempfile.TemporaryDirectory(prefix="powerhouse-observer-intake-test-") as temp:
            base = Path(temp)
            now = int(time.time())
            service = intake_module.ObserverIntake(settings(base))
            try:
                invalid_signature, _ = create_registration(
                    base, "invalid-signature", 31, metrics_one.url, p2p.port, now, now + 3600
                )
                replacement = "A" if invalid_signature["signature_b64"][0] != "A" else "B"
                invalid_signature["signature_b64"] = replacement + invalid_signature["signature_b64"][1:]
                assert_rejected(service, invalid_signature, "verification_failed")

                wrong_chain, _ = create_registration(
                    base, "wrong-chain", 32, metrics_one.url, p2p.port, now, now + 3600
                )
                wrong_chain["chain_id"] = 1
                assert_rejected(service, wrong_chain, "verification_failed")

                wrong_peer, _ = create_registration(
                    base, "wrong-peer", 33, metrics_one.url, p2p.port, now, now + 3600
                )
                wrong_peer["peer_id"] = invalid_signature["peer_id"]
                assert_rejected(service, wrong_peer, "verification_failed")

                unknown_field, _ = create_registration(
                    base, "unknown-field", 34, metrics_one.url, p2p.port, now, now + 3600
                )
                unknown_field["validator_quorum"] = True
                assert_rejected(service, unknown_field, "verification_failed")

                expired, _ = create_registration(
                    base,
                    "expired-observer",
                    35,
                    metrics_one.url,
                    p2p.port,
                    now - 7200,
                    now - 3600,
                )
                assert_rejected(service, expired, "verification_failed")

                mismatch, mismatch_identity = create_registration(
                    base, "identity-mismatch", 36, metrics_one.url, p2p.port, now, now + 3600
                )
                metrics_one.body = metrics_body("other-node", mismatch_identity)
                assert_rejected(service, mismatch, "identity_mismatch")

                valid, valid_identity = create_registration(
                    base, "external-canary", 37, metrics_one.url, p2p.port, now, now + 3600
                )
                metrics_one.body = metrics_body("external-canary", valid_identity, peers=4)
                promoted, response_code = service.submit(valid, background=False)
                assert response_code == 202
                assert promoted["status"] == "promoted"
                assert promoted["checks"]["connected_peers"] == 4
                assert promoted["registry_revision"]
                original_registry = service.settings.registry.read_bytes()
                assert (service.settings.registry.stat().st_mode & 0o777) == 0o640
                registry = json.loads(original_registry)
                assert registry["chain_id"] == 177155
                assert len(registry["registrations"]) == 1

                duplicate, duplicate_code = service.submit(valid, background=False)
                assert duplicate_code == 200
                assert duplicate["duplicate"] is True
                assert duplicate["tracking_id"] == promoted["tracking_id"]

                imported_base = base / "imported"
                imported_base.mkdir()
                imported_settings = settings(imported_base)
                imported_settings.registry.write_bytes(service.settings.registry.read_bytes())
                imported_service = intake_module.ObserverIntake(imported_settings)
                try:
                    imported, imported_code = imported_service.submit(valid, background=False)
                    assert imported_code == 200
                    assert imported["status"] == "promoted"
                    assert imported["duplicate"] is True
                    assert imported["checks"]["registry_import"] == "verified"
                finally:
                    imported_service.close()

                replay, _ = create_registration(
                    base,
                    "external-canary",
                    37,
                    metrics_one.url,
                    p2p.port,
                    now,
                    now + 3600,
                    operator="Changed Operator",
                )
                try:
                    service.submit(replay, background=False)
                except intake_module.IntakeError as error:
                    assert error.code == "replayed_registration"
                else:
                    raise AssertionError("signed replay was accepted")

                refreshed, refreshed_identity = create_registration(
                    base,
                    "external-canary",
                    37,
                    metrics_one.url,
                    p2p.port,
                    now + 1,
                    now + 3601,
                )
                metrics_one.body = metrics_body("external-canary", refreshed_identity, peers=5)
                original_replace = service._replace_registry

                def fail_replace(source: Path, destination: Path) -> None:
                    raise OSError("injected promotion failure")

                service._replace_registry = fail_replace
                rolled_back, _ = service.submit(refreshed, background=False)
                assert rolled_back["status"] == "approved"
                assert rolled_back["retryable"] is True
                assert rolled_back["error"]["code"] == "registry_update_failed"
                assert service.settings.registry.read_bytes() == original_registry
                service._replace_registry = original_replace
                retried, retry_code = service.retry(rolled_back["tracking_id"], background=False)
                assert retry_code == 202
                assert retried["status"] == "promoted"
                assert service.settings.registry.read_bytes() != original_registry

                limiter = intake_module.ObserverIntake(settings(base / "limited", rate_limit=2))
                try:
                    limiter.rate_limit("203.0.113.10")
                    limiter.rate_limit("203.0.113.10")
                    try:
                        limiter.rate_limit("203.0.113.10")
                    except intake_module.IntakeError as error:
                        assert error.code == "rate_limited"
                        assert error.status == 429
                    else:
                        raise AssertionError("rate limit did not reject abuse")
                finally:
                    limiter.close()

                server = intake_module.IntakeServer(("127.0.0.1", 0), service)
                thread = threading.Thread(target=server.serve_forever, daemon=True)
                thread.start()
                root = f"http://127.0.0.1:{server.server_port}"
                try:
                    http_valid, http_identity = create_registration(
                        base,
                        "http-canary",
                        38,
                        metrics_two.url,
                        p2p.port,
                        now,
                        now + 3600,
                    )
                    metrics_two.body = metrics_body("http-canary", http_identity, peers=2)
                    response_code, queued = http_json(
                        f"{root}/observer-registrations",
                        "POST",
                        http_valid,
                        "https://mfenx.com",
                    )
                    assert response_code == 202
                    terminal = wait_for_status(
                        f"{root}/observer-registrations/{queued['tracking_id']}"
                    )
                    assert terminal["status"] == "promoted"
                    assert terminal["checks"]["identity"] == "verified"
                    http_registration = base / "http-canary.registration.json"
                    http_registration.write_text(json.dumps(http_valid))
                    cli_submission = json.loads(
                        run(
                            "observer",
                            "submit",
                            str(http_registration),
                            "--no-probe",
                            "--intake-url",
                            f"{root}/observer-registrations",
                            "--json",
                        ).stdout
                    )
                    assert cli_submission["admission"]["duplicate"] is True
                    assert cli_submission["admission"]["tracking_id"] == queued["tracking_id"]
                    cli_status = json.loads(
                        run(
                            "observer",
                            "status",
                            queued["tracking_id"],
                            "--intake-url",
                            f"{root}/observer-registrations",
                            "--json",
                        ).stdout
                    )
                    assert cli_status["status"] == "promoted"

                    response_code, rejected_origin = http_json(
                        f"{root}/observer-registrations",
                        "POST",
                        http_valid,
                        "https://attacker.example",
                    )
                    assert response_code == 403
                    assert rejected_origin["error"]["code"] == "origin_rejected"

                    malformed = Request(
                        f"{root}/observer-registrations",
                        data=b"{broken",
                        method="POST",
                        headers={"Content-Type": "application/json"},
                    )
                    try:
                        urlopen(malformed, timeout=5)
                    except HTTPError as error:
                        assert error.code == 400
                        assert json.loads(error.read())["error"]["code"] == "invalid_json"
                    else:
                        raise AssertionError("malformed JSON was accepted")

                    oversized = Request(
                        f"{root}/observer-registrations",
                        data=b" " * (intake_module.MAX_BODY_BYTES + 1),
                        method="POST",
                        headers={"Content-Type": "application/json"},
                    )
                    try:
                        urlopen(oversized, timeout=5)
                    except HTTPError as error:
                        assert error.code == 413
                        assert json.loads(error.read())["error"]["code"] == "body_too_large"
                    else:
                        raise AssertionError("oversized request was accepted")
                finally:
                    server.shutdown()
                    server.server_close()
                    thread.join(timeout=2)
            finally:
                service.close()
    finally:
        metrics_one.close()
        metrics_two.close()
        p2p.close()

    print("test_observer_intake: PASS")


if __name__ == "__main__":
    os.chdir(ROOT)
    main()
