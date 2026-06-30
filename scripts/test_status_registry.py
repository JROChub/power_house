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


def campaign_state(updated_at: str) -> dict:
    return {
        "schema": "power-house-reliability-campaign-v1",
        "campaign_id": "rel_20260620T000000Z",
        "status": "running",
        "phase": "soak",
        "updated_at": updated_at,
        "duration_seconds": 259200,
        "elapsed_seconds": 3600,
        "remaining_seconds": 255600,
        "progress_percent": 1.3889,
        "sample_count": 60,
        "successful_samples": 60,
        "failed_samples": 0,
        "max_consecutive_failures": 0,
        "uptime_percent": 100.0,
        "rpc": {"requests": 210, "errors": 0, "p95_ms": 45.2},
        "network": {"validators_healthy": 3, "validators_total": 3},
        "drills": {"scheduled": 4, "completed": 0, "failed": 0, "items": []},
        "evidence": {"events": 61, "head_sha256": "a" * 64},
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
    assert "powerhouse-observer-intake" in deployment
    assert "RELIABILITY_CAMPAIGN_STATE" in deployment
    assert "/var/lib/powerhouse/reliability" in deployment
    assert "OBSERVER_REGISTRY_URL" in deployment
    assert "validator-registry.json" in deployment
    assert "install -d -m 0755 /usr/local/lib/powerhouse" in deployment
    assert "/var/lib/powerhouse/monitoring/observer-registry.json" in deployment
    assert "printf /etc/powerhouse/observer-registry.json" not in deployment
    website = (ROOT / "publicpower" / "app.js").read_text()
    assert "validators_total) || 3" not in website
    nginx = (ROOT / "infra" / "monitoring" / "nginx-mfenx-rpc.conf").read_text()
    assert "/observer-probe" in nginx
    assert "/observer-registrations" in nginx
    assert 'location ~ "^/observer-registrations/obs_[a-f0-9]{32}(/retry)?$"' in nginx
    assert "/observer-intake-healthz" in nginx
    assert "__OBSERVER_INTAKE_UPSTREAM__" in nginx
    provisioner = (ROOT / "scripts" / "provision_digitalocean_rpc.sh").read_text()
    assert "ports:7002,address:0.0.0.0/0" in provisioner
    assert "ports:9195,tag:$TAG" in provisioner
    terraform = (ROOT / "infra" / "terraform" / "digitalocean" / "main.tf").read_text()
    assert 'port_range       = "7002"' in terraform
    assert 'port_range  = "9195"' in terraform
    intake_unit = (ROOT / "infra" / "monitoring" / "powerhouse-observer-intake.service").read_text()
    assert "User=powerhouse-intake" in intake_unit
    assert "ReadWritePaths=/var/lib/powerhouse/observer-intake" in intake_unit
    assert "/etc/powerhouse" not in next(
        line for line in intake_unit.splitlines() if line.startswith("ReadWritePaths=")
    )
    boot_unit = (ROOT / "infra" / "systemd" / "powerhouse-observer-boot.service").read_text()
    assert "powerhouse-common.env" not in boot_unit
    assert "blackbox-observer-intake" in prometheus
    alerts = (ROOT / "infra" / "monitoring" / "powerhouse-alerts.yml").read_text()
    assert "PowerHouseObserverIntakeUnavailable" in alerts

    module = load_module()
    now = datetime.now(timezone.utc)
    with tempfile.TemporaryDirectory(prefix="powerhouse-status-registry-") as temp:
        state_path = Path(temp) / "state.json"
        observer_path = Path(temp) / "observer-state.json"
        campaign_path = Path(temp) / "campaign-status.json"
        module.REGISTRY_STATE_PATH = str(state_path)
        module.OBSERVER_STATE_PATH = str(observer_path)
        module.CAMPAIGN_STATE_PATH = str(campaign_path)
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
        assert snapshot["reliability_campaign"]["status"] == "not_started"

        campaign_path.write_text(json.dumps(campaign_state(now.isoformat())))
        snapshot = module.snapshot()
        assert snapshot["reliability_campaign"]["status"] == "running"
        assert snapshot["reliability_campaign"]["fresh"] is True
        assert snapshot["reliability_campaign"]["sample_count"] == 60

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
        campaign_path.write_text(json.dumps(campaign_state(stale.isoformat())))
        assert module.campaign_health()["status"] == "stalled"
        complete = campaign_state(stale.isoformat())
        complete["status"] = "passed"
        complete["phase"] = "complete"
        complete["evidence"]["final_report_sha256"] = "b" * 64
        campaign_path.write_text(json.dumps(complete))
        completed_campaign = module.campaign_health()
        assert completed_campaign["status"] == "passed"
        assert completed_campaign["fresh"] is True

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
