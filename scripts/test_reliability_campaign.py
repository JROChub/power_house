#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
CAMPAIGN = ROOT / "infra" / "monitoring" / "reliability_campaign.py"


def load_module():
    spec = importlib.util.spec_from_file_location("powerhouse_reliability_campaign", CAMPAIGN)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def config_value(base: Path) -> dict:
    nodes = []
    for index in range(1, 4):
        nodes.append(
            {
                "name": f"validator-{index}",
                "target": f"root@192.0.2.{index}",
                "service": f"powerhouse-node@validator-{index}.service",
                "state_path": f"/var/lib/powerhouse/validator-{index}/native_chain_state.json",
                "observer_registry_path": f"/var/lib/powerhouse/observer-{index}.json",
            }
        )
    return {
        "schema": "power-house-reliability-config-v1",
        "state_dir": str(base / "state"),
        "duration_seconds": 120,
        "sample_interval_seconds": 5,
        "burst_interval_seconds": 10,
        "burst_requests": 3,
        "recovery_timeout_seconds": 15,
        "expected_chain_id": 177155,
        "expected_release": "0.3.11",
        "rpc_url": "https://rpc.example",
        "status_url": "https://rpc.example/network-status.json",
        "intake_url": "https://rpc.example/observer-intake-healthz",
        "primary_node": "validator-1",
        "nodes": nodes,
        "ssh_options": ["-F", "/dev/null", "-o", "BatchMode=yes"],
        "publish_targets": [item["target"] for item in nodes],
        "publish_path": "/var/lib/powerhouse/reliability/campaign-status.json",
        "drills": [
            {"id": "validator-test", "kind": "validator_failover", "offset_seconds": 30}
        ],
    }


def fake_node(name: str) -> dict:
    return {
        "name": name,
        "version": "0.3.11",
        "binary_sha256": "1" * 64,
        "validator_registry_sha256": "2" * 64,
        "state_sha256": "3" * 64,
        "observer_registry_sha256": "4" * 64,
        "service": "active",
        "health": {
            "status": "ok",
            "chain_id": 177155,
            "finalized_block": 42,
            "finalized_hash": "0x" + "5" * 64,
        },
        "active_alerts": [],
    }


def install_fakes(campaign) -> None:
    def http(url, data=None, timeout=8):
        if data is not None:
            method = json.loads(data)["method"]
            result = {
                "eth_chainId": hex(177155),
                "eth_blockNumber": hex(42),
                "web3_clientVersion": "power-house/0.3.11/finalized-native-rpc",
            }[method]
            return {"jsonrpc": "2.0", "id": 1, "result": result}, 12.5
        if url.endswith("network-status.json"):
            return {
                "status": "operational",
                "release": "0.3.11",
                "validators_healthy": 3,
                "validators_total": 3,
                "validator_registry": {"verified": True},
                "observer_registry": {"verified": True},
                "observer_peers": {"healthy": 1, "total": 1, "connected": 3},
                "block_height": 42,
            }, 9.5
        return {"status": "ok"}, 8.5

    campaign._http_json = http
    campaign.audit_node = lambda node: fake_node(node.name)
    campaign.publish = lambda: None


def verify_event_chain(module, path: Path) -> None:
    previous = "0" * 64
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        event = json.loads(line)
        assert event["sequence"] == number
        assert event["previous_hash"] == previous
        event_hash = event.pop("event_hash")
        assert event_hash == module.digest_json(event)
        previous = event_hash


def main() -> None:
    module = load_module()
    with tempfile.TemporaryDirectory(prefix="powerhouse-reliability-test-") as temp:
        base = Path(temp)
        config_path = base / "config.json"
        config_path.write_text(json.dumps(config_value(base)))
        config = module.Config.load(config_path)
        campaign = module.Campaign(config)
        install_fakes(campaign)

        sample = campaign.collect_sample()
        assert sample["ok"] is True
        assert len(sample["nodes"]) == 3
        campaign.apply_sample(sample)
        campaign.save()
        status = campaign.public_status()
        assert status["status"] == "running"
        assert status["sample_count"] == 1
        assert status["uptime_percent"] == 100.0
        assert status["network"]["validators_healthy"] == 3
        assert status["evidence"]["events"] == 1

        campaign._drill_action = lambda kind: {
            "passed": True,
            "recovery_seconds": 5.25,
            "requests": 20,
            "errors": 0,
            "service_active": True,
        }
        drill = campaign.perform_drill(campaign.state["drills"][0])
        assert drill["status"] == "passed"
        assert drill["recovery_seconds"] == 5.25
        verify_event_chain(module, campaign.events_path)

        mismatch = fake_node("validator-3")
        mismatch["validator_registry_sha256"] = "9" * 64
        campaign.audit_node = lambda node: (
            mismatch if node.name == "validator-3" else fake_node(node.name)
        )
        rejected = campaign.collect_sample()
        assert rejected["ok"] is False
        assert "validator validator_registry_sha256 values differ" in rejected["errors"]

        campaign.audit_node = lambda node: {
            **fake_node(node.name),
            "active_alerts": ["PowerHouseValidatorDown"] if node.name == "validator-1" else [],
        }
        alerted = campaign.collect_sample()
        assert alerted["ok"] is False
        assert "active Prometheus alerts: PowerHouseValidatorDown" in alerted["errors"]

        campaign.state["last_sample_unix"] -= 20
        before_failed = campaign.state["failed_samples"]
        campaign.audit_node = lambda node: fake_node(node.name)
        campaign.apply_sample(campaign.collect_sample())
        assert campaign.state["failed_samples"] > before_failed
        assert json.loads(campaign.events_path.read_text().splitlines()[-2])["kind"] == "telemetry_gap"

        campaign.finalize()
        assert campaign.report_path.exists()
        assert campaign.manifest_path.exists()
        assert campaign.state["final_report_sha256"]
        verify_event_chain(module, campaign.events_path)

        campaign.state["drills"][0]["status"] = "running"
        module.atomic_json(campaign.state_path, campaign.state)
        resumed = module.Campaign(config)
        assert resumed.state["drills"][0]["status"] == "failed"
        assert "restarted during drill" in resumed.state["drills"][0]["detail"]

        changed = config_value(base)
        changed["expected_release"] = "0.3.12"
        changed_path = base / "changed.json"
        changed_path.write_text(json.dumps(changed))
        try:
            module.Campaign(module.Config.load(changed_path))
        except RuntimeError as error:
            assert "configuration changed" in str(error)
        else:
            raise AssertionError("changed campaign configuration resumed existing state")

        invalid = config_value(base / "invalid")
        invalid["nodes"][0]["target"] = "root@example;rm"
        invalid_path = base / "invalid.json"
        invalid_path.write_text(json.dumps(invalid))
        try:
            module.Config.load(invalid_path)
        except ValueError as error:
            assert "unsafe" in str(error)
        else:
            raise AssertionError("unsafe SSH target was accepted")

    html = (ROOT / "publicpower" / "campaign.html").read_text()
    javascript = (ROOT / "publicpower" / "campaign.js").read_text()
    main_html = (ROOT / "publicpower" / "index.html").read_text()
    main_js = (ROOT / "publicpower" / "app.js").read_text()
    assert 'id="campaign-state"' in html
    assert 'id="drill-list"' in html
    assert "reliability_campaign" in javascript
    assert "reliability_campaign" not in main_js
    assert "campaign.html" not in main_html
    unit = (ROOT / "infra" / "systemd" / "powerhouse-reliability-campaign.service").read_text()
    assert "ProtectSystem=strict" in unit
    assert "ProtectHome=read-only" in unit
    assert "ReadWritePaths=%h/.local/state/powerhouse/reliability" in unit
    subprocess.run(["node", "--check", str(ROOT / "publicpower" / "campaign.js")], check=True)
    print("test_reliability_campaign: PASS")


if __name__ == "__main__":
    main()
