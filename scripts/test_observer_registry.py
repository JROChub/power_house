#!/usr/bin/env python3

from __future__ import annotations

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target" / "debug" / "julian"
RECONCILER = ROOT / "infra" / "monitoring" / "observer_registry.py"


class MetricsServer:
    def __init__(self) -> None:
        self.body = ""
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path != "/metrics":
                    self.send_error(404)
                    return
                encoded = owner.body.encode()
                self.send_response(200)
                self.send_header("Content-Type", "text/plain; version=0.0.4")
                self.send_header("Content-Length", str(len(encoded)))
                self.end_headers()
                self.wfile.write(encoded)

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


class RegistryServer:
    def __init__(self) -> None:
        self.body = b""
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path != "/observer-registry.json":
                    self.send_error(404)
                    return
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(owner.body)))
                self.end_headers()
                self.wfile.write(owner.body)

            def log_message(self, format, *args):
                return

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.server.server_port}/observer-registry.json"

    def close(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(BINARY), *args],
        check=check,
        capture_output=True,
        text=True,
    )


def identity(key: Path) -> dict:
    return json.loads(run("key-info", str(key), "--json").stdout)


def metric_body(node: dict, peer_links: int = 2) -> str:
    return (
        "# TYPE powerhouse_node_identity gauge\n"
        "powerhouse_node_identity{"
        f'node_id="{node["node_id"]}",'
        f'peer_id="{node["peer_id"]}",'
        f'public_key_b64="{node["public_key_b64"]}",'
        'chain_id="177155"} 1\n'
        "# TYPE powerhouse_connected_peers gauge\n"
        f"powerhouse_connected_peers {peer_links}\n"
    )


def main() -> None:
    now = int(time.time())
    servers = [MetricsServer() for _ in range(2)]
    registry_server = RegistryServer()
    try:
        with tempfile.TemporaryDirectory(prefix="powerhouse-observer-registry-test-") as temp:
            base = Path(temp)
            missing_state = base / "missing-state.json"
            missing_discovery = base / "missing-discovery.json"
            missing = subprocess.run(
                [
                    sys.executable,
                    str(RECONCILER),
                    "--registry",
                    str(base / "missing-observers.json"),
                    "--binary",
                    str(BINARY),
                    "--state",
                    str(missing_state),
                    "--observer-discovery",
                    str(missing_discovery),
                    "--timeout",
                    "1",
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            assert "not configured" in missing.stdout
            health = json.loads(missing_state.read_text())
            assert health["configured"] is False
            assert health["observers_total"] == 0
            assert json.loads(missing_discovery.read_text()) == []

            registrations = []
            nodes = []
            for index, server in enumerate(servers, start=1):
                key = base / f"observer-{index}.key"
                key.write_bytes(bytes([20 + index]) * 32)
                info = identity(key)
                node = {
                    "node_id": f"observer-{index}",
                    "region": ["lax1", "fra1"][index - 1],
                    **info,
                }
                nodes.append(node)
                registration = base / f"observer-{index}.registration.json"
                run(
                    "observer-registry",
                    "create",
                    "--key",
                    str(key),
                    "--node-id",
                    node["node_id"],
                    "--operator",
                    "Public Observer",
                    "--region",
                    node["region"],
                    "--p2p-address",
                    f"/ip4/127.0.0.1/tcp/{7100 + index}/p2p/{info['peer_id']}",
                    "--metrics-url",
                    server.url,
                    "--issued-at",
                    str(now),
                    "--valid-until",
                    str(now + 3600),
                    "--output",
                    str(registration),
                )
                registrations.append(registration)
                server.body = metric_body(node, peer_links=index)

                if index == 1:
                    doctor = json.loads(
                        run(
                            "observer",
                            "doctor",
                            "--key",
                            str(key),
                            "--node-id",
                            node["node_id"],
                            "--public-host",
                            "127.0.0.1",
                            "--p2p-port",
                            str(7100 + index),
                            "--metrics-port",
                            str(server.server.server_port),
                            "--no-probe",
                            "--json",
                        ).stdout
                    )
                    assert doctor["schema"] == "power-house-observer-doctor-v1"
                    assert doctor["peer_id"] == info["peer_id"]
                    checks = {item["name"]: item for item in doctor["checks"]}
                    assert checks["node key"]["status"] == "OK"
                    assert checks["local metrics identity"]["status"] == "OK"

                    friendly = base / "observer-friendly.registration.json"
                    run(
                        "observer",
                        "register",
                        "--key",
                        str(key),
                        "--node-id",
                        node["node_id"],
                        "--operator",
                        "Public Observer",
                        "--region",
                        node["region"],
                        "--public-host",
                        "127.0.0.1",
                        "--p2p-port",
                        str(7100 + index),
                        "--metrics-port",
                        str(server.server.server_port),
                        "--output",
                        str(friendly),
                    )
                    assert json.loads(friendly.read_text())["schema"] == (
                        "power-house-observer-registration-v1"
                    )

                    setup = json.loads(
                        run(
                            "observer",
                            "setup",
                            "--key",
                            str(key),
                            "--node-id",
                            node["node_id"],
                            "--operator",
                            "Public Observer",
                            "--region",
                            node["region"],
                            "--public-host",
                            "127.0.0.1",
                            "--p2p-port",
                            str(7100 + index),
                            "--metrics-port",
                            str(server.server.server_port),
                            "--output",
                            str(base / "observer-setup.registration.json"),
                            "--no-probe",
                            "--json",
                        ).stdout
                    )
                    assert "/tcp/7002/p2p/" in setup["start_command"]

            registry = base / "observer-registry.json"
            assemble = ["observer-registry", "assemble", "--output", str(registry)]
            for registration in registrations:
                assemble.extend(["--registration", str(registration)])
            run(*assemble)

            verified = json.loads(
                run(
                    "observer-registry",
                    "verify",
                    str(registry),
                    "--now",
                    str(now),
                    "--json",
                ).stdout
            )
            assert verified["verified"] is True
            assert verified["observers_verified"] == 2

            registry_server.body = registry.read_bytes()
            replica = base / "replica-observer-registry.json"
            replica_state = base / "replica-state.json"
            replica_discovery = base / "replica-discovery.json"
            replica_env = {
                **os.environ,
                "OBSERVER_REGISTRY_URL": registry_server.url,
                "OBSERVER_REGISTRY_PATH": str(replica),
            }
            subprocess.run(
                [
                    sys.executable,
                    str(RECONCILER),
                    "--binary",
                    str(BINARY),
                    "--state",
                    str(replica_state),
                    "--observer-discovery",
                    str(replica_discovery),
                    "--timeout",
                    "1",
                ],
                env=replica_env,
                check=True,
                capture_output=True,
                text=True,
            )
            verified_replica = replica.read_bytes()
            assert json.loads(verified_replica) == json.loads(registry.read_bytes())

            tampered_registry = json.loads(registry.read_text())
            tampered_registry["registrations"][0]["signature_b64"] = "invalid"
            registry_server.body = json.dumps(tampered_registry).encode()
            failed_sync = subprocess.run(
                [
                    sys.executable,
                    str(RECONCILER),
                    "--binary",
                    str(BINARY),
                    "--state",
                    str(replica_state),
                    "--observer-discovery",
                    str(replica_discovery),
                    "--timeout",
                    "1",
                ],
                env=replica_env,
                check=False,
                capture_output=True,
                text=True,
            )
            assert failed_sync.returncode == 2
            assert replica.read_bytes() == verified_replica

            refresh = base / "observer-1.refresh.registration.json"
            refresh_registry = base / "observer-refresh-registry.json"
            run(
                "observer-registry",
                "register",
                "--key",
                str(base / "observer-1.key"),
                "--node-id",
                nodes[0]["node_id"],
                "--operator",
                "Public Observer",
                "--region",
                nodes[0]["region"],
                "--public-host",
                "127.0.0.1",
                "--p2p-port",
                "7101",
                "--metrics-url",
                servers[0].url,
                "--issued-at",
                str(now),
                "--valid-until",
                str(now + 3600),
                "--output",
                str(refresh),
                "--registry",
                str(registry),
                "--registry-output",
                str(refresh_registry),
            )
            refreshed = json.loads(
                run(
                    "observer-registry",
                    "verify",
                    str(refresh_registry),
                    "--now",
                    str(now),
                    "--json",
                ).stdout
            )
            assert refreshed["verified"] is True
            assert refreshed["observers_verified"] == 2

            state = base / "state.json"
            discovery = base / "observers.json"
            reconcile = [
                sys.executable,
                str(RECONCILER),
                "--registry",
                str(registry),
                "--binary",
                str(BINARY),
                "--state",
                str(state),
                "--observer-discovery",
                str(discovery),
                "--timeout",
                "1",
            ]
            subprocess.run(reconcile, check=True, capture_output=True, text=True)
            health = json.loads(state.read_text())
            assert health["registry_verified"] is True
            assert health["observers_total"] == 2
            assert health["observers_healthy"] == 2
            assert health["observer_connections"] == 3
            assert all(item["identity_verified"] for item in health["observers"])
            assert len(json.loads(discovery.read_text())) == 2
            assert stat.S_IMODE(discovery.stat().st_mode) == 0o644
            assert stat.S_IMODE(state.stat().st_mode) == 0o640

            servers[0].body = metric_body(
                {**nodes[0], "public_key_b64": nodes[1]["public_key_b64"]},
                peer_links=1,
            )
            subprocess.run(reconcile, check=True, capture_output=True, text=True)
            health = json.loads(state.read_text())
            assert health["registry_verified"] is True
            assert health["observers_healthy"] == 1
            assert health["observers"][0]["identity_verified"] is False
    finally:
        for server in servers:
            server.close()
        registry_server.close()

    print("test_observer_registry: PASS")


if __name__ == "__main__":
    os.chdir(ROOT)
    main()
