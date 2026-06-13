#!/usr/bin/env python3

from __future__ import annotations

from datetime import datetime, timedelta, timezone
import importlib.util
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


def main() -> None:
    prometheus = (ROOT / "infra" / "monitoring" / "prometheus.yml").read_text()
    assert "/etc/prometheus/file_sd/powerhouse-validators.json" in prometheus
    assert "/etc/prometheus/file_sd/powerhouse-systems.json" in prometheus
    assert "__NODE" not in prometheus
    deployment = (ROOT / "scripts" / "deploy_monitoring_stack.sh").read_text()
    assert "powerhouse-validator-registry.timer" in deployment
    assert "validator-registry.json" in deployment
    website = (ROOT / "publicpower" / "app.js").read_text()
    assert "validators_total) || 3" not in website

    module = load_module()
    now = datetime.now(timezone.utc)
    with tempfile.TemporaryDirectory(prefix="powerhouse-status-registry-") as temp:
        state_path = Path(temp) / "state.json"
        module.REGISTRY_STATE_PATH = str(state_path)
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
        assert snapshot["validator_registry"] == {
            "fresh": True,
            "identity_verified": 4,
            "verified": True,
        }

        stale = now - timedelta(minutes=5)
        state_path.write_text(json.dumps(registry_state(stale.isoformat())))
        snapshot = module.snapshot()
        assert snapshot["status"] == "outage"
        assert snapshot["validator_registry"]["fresh"] is False

    print("test_status_registry: PASS")


if __name__ == "__main__":
    main()
