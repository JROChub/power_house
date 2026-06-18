#!/usr/bin/env python3

from __future__ import annotations

from datetime import datetime, timedelta, timezone
import importlib.util
import ipaddress
import json
from pathlib import Path
import tempfile


ROOT = Path(__file__).resolve().parents[1]
STATUS_API = ROOT / "infra" / "monitoring" / "status_api.py"


def load_module():
    spec = importlib.util.spec_from_file_location("powerhouse_status_api", STATUS_API)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def registry_state(generated_at: str) -> dict:
    validators = [
        {
            "node_id": f"validator-{index}",
            "healthy": True,
            "identity_verified": True,
            "system_metrics_reachable": True,
            "peer_links": 3,
        }
        for index in range(1, 5)
    ]
    return {
        "schema": "power-house-validator-registry-health-v1",
        "chain_id": 177155,
        "generated_at": generated_at,
        "registry_verified": True,
        "validators_total": 4,
        "validators_healthy": 4,
        "peer_link_observations": 12,
        "validators": validators,
        "error": None,
    }


def observer_state(generated_at: str) -> dict:
    observers = [
        {
            "node_id": "observer-lax-1",
            "healthy": True,
            "identity_verified": True,
            "metrics_reachable": True,
            "system_metrics_reachable": True,
            "peer_links": 5,
        },
        {
            "node_id": "observer-fra-1",
            "healthy": False,
            "identity_verified": False,
            "metrics_reachable": True,
            "system_metrics_reachable": True,
            "peer_links": 0,
        },
    ]
    return {
        "schema": "power-house-observer-registry-health-v1",
        "chain_id": 177155,
        "configured": True,
        "generated_at": generated_at,
        "registry_verified": True,
        "observers_total": 2,
        "observers_healthy": 1,
        "observer_connections": 5,
        "observers": observers,
        "error": None,
    }


def main() -> None:
    prometheus = (ROOT / "infra" / "monitoring" / "prometheus.yml").read_text()
    assert "/etc/prometheus/file_sd/powerhouse-validators.json" in prometheus
    assert "/etc/prometheus/file_sd/powerhouse-observers.json" in prometheus
    assert "/etc/prometheus/file_sd/powerhouse-systems.json" in prometheus
    assert "__NODE" not in prometheus
    deployment = (ROOT / "scripts" / "deploy_monitoring_stack.sh").read_text()
    assert "powerhouse-validator-registry.timer" in deployment
    assert "powerhouse-observer-registry.timer" in deployment
    assert "validator-registry.json" in deployment
    website = (ROOT / "publicpower" / "app.js").read_text()
    assert "validators_total) || 3" not in website
    nginx = (ROOT / "infra" / "monitoring" / "nginx-mfenx-rpc.conf").read_text()
    assert "/observer-probe" in nginx

    module = load_module()
    now = datetime.now(timezone.utc)
    with tempfile.TemporaryDirectory(prefix="powerhouse-status-registry-") as temp:
        state_path = Path(temp) / "state.json"
        observer_path = Path(temp) / "observer-state.json"
        module.REGISTRY_STATE_PATH = str(state_path)
        module.OBSERVER_STATE_PATH = str(observer_path)
        module.query = lambda expression: (
            1.0 if "probe_success" in expression and "avg_over_time" not in expression else 0.999
        )
        module.fetch_json = lambda url, data=None, headers=None: {
            "data": {"startTime": (now - timedelta(days=2)).isoformat()}
        }
        module.rpc = lambda method: {
            "eth_chainId": hex(177155),
            "eth_blockNumber": hex(42),
            "web3_clientVersion": "power-house/test",
        }[method]

        state_path.write_text(json.dumps(registry_state(now.isoformat())))
        snapshot = module.snapshot()
        assert snapshot["status"] == "operational"
        assert snapshot["validators_healthy"] == 4
        assert snapshot["validators_total"] == 4
        assert snapshot["peer_connections"] == 12
        assert snapshot["validator_peer_links"] == 12
        assert snapshot["public_peer_connections"] == 0
        assert snapshot["observer_peers"] == {
            "configured": False,
            "connected": 0,
            "fresh": False,
            "healthy": 0,
            "total": 0,
        }
        assert snapshot["validator_registry"] == {
            "fresh": True,
            "identity_verified": 4,
            "verified": True,
        }

        observer_path.write_text(json.dumps(observer_state(now.isoformat())))
        snapshot = module.snapshot()
        assert snapshot["status"] == "operational"
        assert snapshot["peer_connections"] == 12
        assert snapshot["public_peer_connections"] == 5
        assert snapshot["observer_peers"] == {
            "configured": True,
            "connected": 5,
            "fresh": True,
            "healthy": 1,
            "total": 2,
        }
        assert snapshot["observer_registry"] == {
            "configured": True,
            "fresh": True,
            "identity_verified": 1,
            "verified": True,
        }

        stale = now - timedelta(minutes=5)
        state_path.write_text(json.dumps(registry_state(stale.isoformat())))
        snapshot = module.snapshot()
        assert snapshot["status"] == "outage"
        assert snapshot["validator_registry"]["fresh"] is False

        try:
            module.public_addresses_for_host("127.0.0.1")
        except ValueError as error:
            assert "non-public" in str(error)
        else:
            raise AssertionError("private observer probe target was accepted")

        module.public_addresses_for_host = lambda host: [ipaddress.ip_address("8.8.8.8")]
        module.probe_metrics = lambda host, port, timeout: {
            "reachable": True,
            "identity_found": True,
            "identity": {"node_id": "observer-lax-1"},
            "connected_peers": 2,
            "error": None,
        }
        module.probe_tcp = lambda host, port, timeout: {
            "reachable": True,
            "error": None,
        }
        probe = module.observer_probe("host=observer.example&metrics_port=9102&p2p_port=7001")
        assert probe["schema"] == "power-house-observer-probe-v1"
        assert probe["ok"] is True
        assert probe["metrics"]["identity"]["node_id"] == "observer-lax-1"
        assert probe["target"]["metrics_port"] == 9102

    print("test_status_registry: PASS")


if __name__ == "__main__":
    main()
