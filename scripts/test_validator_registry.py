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
RECONCILER = ROOT / "infra" / "monitoring" / "validator_registry.py"


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
    servers = [MetricsServer() for _ in range(3)]
    try:
        with tempfile.TemporaryDirectory(prefix="powerhouse-registry-test-") as temp:
            base = Path(temp)
            registrations = []
            nodes = []
            for index, server in enumerate(servers, start=1):
                key = base / f"validator-{index}.key"
                key.write_bytes(bytes([index]) * 32)
                info = identity(key)
                node = {
                    "node_id": f"validator-{index}",
                    "region": ["nyc3", "sfo3", "ams3"][index - 1],
                    **info,
                }
                nodes.append(node)
                registration = base / f"validator-{index}.registration.json"
                run(
                    "validator-registry",
                    "create",
                    "--key",
                    str(key),
                    "--node-id",
                    node["node_id"],
                    "--operator",
                    "MFENX LLC",
                    "--region",
                    node["region"],
                    "--p2p-address",
                    f"/ip4/127.0.0.1/tcp/{7000 + index}/p2p/{info['peer_id']}",
                    "--metrics-url",
                    server.url,
                    "--system-metrics-url",
                    server.url,
                    "--issued-at",
                    str(now),
                    "--valid-until",
                    str(now + 3600),
                    "--output",
                    str(registration),
                )
                registrations.append(registration)
                server.body = metric_body(node)

            policy = base / "native-validators.json"
            policy.write_text(
                json.dumps(
                    {
                        "allowlist": [node["public_key_b64"] for node in nodes],
                        "backend": "static",
                    }
                )
            )
            registry = base / "validator-registry.json"
            assemble = [
                "validator-registry",
                "assemble",
                "--policy",
                str(policy),
                "--output",
                str(registry),
            ]
            for registration in registrations:
                assemble.extend(["--registration", str(registration)])
            run(*assemble)
            original_registry = registry.read_text()

            verified = json.loads(
                run(
                    "validator-registry",
                    "verify",
                    str(registry),
                    "--policy",
                    str(policy),
                    "--now",
                    str(now),
                    "--json",
                ).stdout
            )
            assert verified["verified"] is True
            assert verified["validators_verified"] == 3

            refresh = base / "validator-1.refresh.registration.json"
            refresh_registry = base / "validator-refresh-registry.json"
            run(
                "validator-registry",
                "register",
                "--key",
                str(base / "validator-1.key"),
                "--node-id",
                nodes[0]["node_id"],
                "--operator",
                "MFENX LLC",
                "--region",
                nodes[0]["region"],
                "--public-host",
                "127.0.0.1",
                "--p2p-port",
                "7001",
                "--metrics-url",
                servers[0].url,
                "--system-metrics-url",
                servers[0].url,
                "--issued-at",
                str(now),
                "--valid-until",
                str(now + 3600),
                "--output",
                str(refresh),
                "--policy",
                str(policy),
                "--registry",
                str(registry),
                "--registry-output",
                str(refresh_registry),
            )
            refreshed = json.loads(
                run(
                    "validator-registry",
                    "verify",
                    str(refresh_registry),
                    "--policy",
                    str(policy),
                    "--now",
                    str(now),
                    "--json",
                ).stdout
            )
            assert refreshed["verified"] is True
            assert refreshed["validators_verified"] == 3

            state = base / "state.json"
            powerhouse_discovery = base / "powerhouse.json"
            node_discovery = base / "systems.json"
            reconcile = [
                sys.executable,
                str(RECONCILER),
                "--registry",
                str(registry),
                "--policy",
                str(policy),
                "--binary",
                str(BINARY),
                "--state",
                str(state),
                "--powerhouse-discovery",
                str(powerhouse_discovery),
                "--node-discovery",
                str(node_discovery),
                "--timeout",
                "1",
            ]
            subprocess.run(reconcile, check=True, capture_output=True, text=True)
            health = json.loads(state.read_text())
            assert health["registry_verified"] is True
            assert health["validators_total"] == 3
            assert health["validators_healthy"] == 3
            assert health["peer_link_observations"] == 6
            assert all(item["identity_verified"] for item in health["validators"])
            assert len(json.loads(powerhouse_discovery.read_text())) == 3
            assert len(json.loads(node_discovery.read_text())) == 3
            assert stat.S_IMODE(powerhouse_discovery.stat().st_mode) == 0o644
            assert stat.S_IMODE(node_discovery.stat().st_mode) == 0o644
            assert stat.S_IMODE(state.stat().st_mode) == 0o640

            servers[1].body = metric_body(
                {**nodes[1], "peer_id": nodes[0]["peer_id"]}
            )
            subprocess.run(reconcile, check=True, capture_output=True, text=True)
            health = json.loads(state.read_text())
            assert health["registry_verified"] is True
            assert health["validators_healthy"] == 2
            assert health["validators"][1]["identity_verified"] is False
            servers[1].body = metric_body(nodes[1])

            last_good_discovery = powerhouse_discovery.read_text()
            tampered = json.loads(registry.read_text())
            tampered["registrations"][0]["region"] = "fra1"
            registry.write_text(json.dumps(tampered))
            failed = subprocess.run(reconcile, check=False, capture_output=True, text=True)
            assert failed.returncode == 2
            health = json.loads(state.read_text())
            assert health["registry_verified"] is False
            assert powerhouse_discovery.read_text() == last_good_discovery

            registry.write_text(original_registry)
            expired = run(
                "validator-registry",
                "verify",
                str(base / "validator-registry.json"),
                "--policy",
                str(policy),
                "--now",
                str(now + 7200),
                check=False,
            )
            assert expired.returncode != 0
            assert "expired" in expired.stderr
    finally:
        for server in servers:
            server.close()

    print("test_validator_registry: PASS")


if __name__ == "__main__":
    os.chdir(ROOT)
    main()
