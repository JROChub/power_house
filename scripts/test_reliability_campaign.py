#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import fcntl
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
        "probe_attempts": 3,
        "probe_retry_delay_seconds": 0,
        "http_timeout_seconds": 2,
        "ssh_timeout_seconds": 2,
        "max_parallel_probes": 8,
        "max_rpc_p95_ms": 1000,
        "expected_chain_id": 177155,
        "expected_release": "0.3.24",
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
        "version": "0.3.24",
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
                "web3_clientVersion": "power-house/0.3.24/finalized-native-rpc",
            }[method]
            return {"jsonrpc": "2.0", "id": 1, "result": result}, 12.5
        if url.endswith("network-status.json"):
            return {
                "status": "operational",
                "release": "0.3.24",
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
        assert status["evidence"]["events"] >= 1
        assert status["acceptance"]["max_rpc_p95_ms"] == 1000

        healthy_http = campaign._http_json
        rpc_calls = {"eth_chainId": 0}

        def transient_rpc(url, data=None, timeout=None):
            if data is not None and json.loads(data)["method"] == "eth_chainId":
                rpc_calls["eth_chainId"] += 1
                if rpc_calls["eth_chainId"] == 1:
                    raise TimeoutError("transient read timeout")
            return healthy_http(url, data=data, timeout=timeout)

        campaign._http_json = transient_rpc
        recovered = campaign.collect_sample()
        assert recovered["ok"] is True
        assert campaign.state["rpc_errors"] == 0
        assert campaign.state["rpc_attempt_errors"] == 1
        assert campaign.state["rpc_recoveries"] == 1
        assert any(
            item["probe"] == "RPC eth_chainId" and item["attempts"] == 2
            for item in recovered["probe_recoveries"]
        )
        campaign._http_json = healthy_http

        node_calls = {"validator-3": 0}

        def transient_node(node):
            if node.name == "validator-3":
                node_calls["validator-3"] += 1
                if node_calls["validator-3"] == 1:
                    raise TimeoutError("transient SSH timeout")
            return fake_node(node.name)

        campaign.audit_node = transient_node
        recovered_node = campaign.collect_sample()
        assert recovered_node["ok"] is True
        assert [node["name"] for node in recovered_node["nodes"]] == [
            "validator-1",
            "validator-2",
            "validator-3",
        ]
        assert any(
            item["probe"] == "validator audit validator-3" and item["attempts"] == 2
            for item in recovered_node["probe_recoveries"]
        )
        campaign.audit_node = lambda node: fake_node(node.name)

        before_preflight_samples = campaign.state["sample_count"]
        preflight = campaign.preflight(3, 0)
        assert preflight["ok"] is True
        assert preflight["successful"] == 3
        assert preflight["recovered"] == 0
        assert campaign.state["sample_count"] == before_preflight_samples

        confirmed_value = config_value(base / "confirmed-failure")
        confirmed_path = base / "confirmed-failure.json"
        confirmed_path.write_text(json.dumps(confirmed_value))
        confirmed = module.Campaign(module.Config.load(confirmed_path))
        install_fakes(confirmed)
        confirmed_http = confirmed._http_json

        def exhausted_rpc(url, data=None, timeout=None):
            if data is not None and json.loads(data)["method"] == "eth_chainId":
                raise TimeoutError("confirmed read timeout")
            return confirmed_http(url, data=data, timeout=timeout)

        confirmed._http_json = exhausted_rpc
        exhausted = confirmed.collect_sample()
        assert exhausted["ok"] is False
        assert exhausted["errors"] == ["RPC eth_chainId: confirmed read timeout"]
        assert confirmed.state["rpc_errors"] == 1
        assert confirmed.state["rpc_attempt_errors"] == 3

        before_samples = campaign.state["sample_count"]
        campaign.state["last_controller_event_unix"] -= 20
        campaign.record_event("rpc_burst", {"requests": 30, "errors": 0})
        campaign.apply_sample(campaign.collect_sample())
        assert campaign.state["sample_count"] == before_samples + 1
        assert campaign.state["missed_controller_samples"] == 0
        assert not any(
            item["kind"] == "telemetry_gap"
            for item in campaign.public_status()["failures"]["recent"]
        )

        calls = {"count": 0}

        def transient_finality(node):
            attempt = calls["count"] // 3
            calls["count"] += 1
            item = fake_node(node.name)
            if attempt == 0 and node.name == "validator-3":
                item["health"]["finalized_block"] = 43
                item["health"]["finalized_hash"] = "0x" + "6" * 64
            return item

        campaign.audit_node = transient_finality
        original_sleep = module.time.sleep
        module.time.sleep = lambda _seconds: None
        try:
            transient = campaign.collect_sample()
        finally:
            module.time.sleep = original_sleep
        assert transient["ok"] is True
        assert calls["count"] == 6
        assert "finalized state differs across validators" not in transient["errors"]

        def persistent_finality(node):
            item = fake_node(node.name)
            if node.name == "validator-3":
                item["health"]["finalized_block"] = 43
                item["health"]["finalized_hash"] = "0x" + "6" * 64
            return item

        campaign.audit_node = persistent_finality
        original_sleep = module.time.sleep
        module.time.sleep = lambda _seconds: None
        try:
            divergent = campaign.collect_sample()
        finally:
            module.time.sleep = original_sleep
        assert divergent["ok"] is False
        assert "finalized state differs across validators" in divergent["errors"]
        campaign.audit_node = lambda node: fake_node(node.name)

        campaign._drill_action = lambda kind: {
            "passed": True,
            "recovery_seconds": 5.25,
            "requests": 20,
            "errors": 0,
            "service_active": True,
        }
        collect_sample = campaign.collect_sample
        healthy_sample = collect_sample()
        converging_sample = json.loads(json.dumps(healthy_sample))
        converging_sample["ok"] = False
        converging_sample["errors"] = ["validator telemetry still converging"]
        recovery_samples = iter([healthy_sample, converging_sample, healthy_sample])
        campaign.collect_sample = lambda: next(recovery_samples)
        original_sleep = module.time.sleep
        module.time.sleep = lambda _seconds: None
        try:
            drill = campaign.perform_drill(campaign.state["drills"][0])
        finally:
            module.time.sleep = original_sleep
        assert drill["status"] == "passed"
        assert drill["recovery_seconds"] >= 0
        events = [json.loads(line) for line in campaign.events_path.read_text().splitlines()]
        assert any(event["kind"] == "drill_recovery_probe" for event in events)
        completed = next(event for event in reversed(events) if event["kind"] == "drill_completed")
        assert completed["data"]["result"]["service_recovery_seconds"] == 5.25
        assert completed["data"]["result"]["recovery_probes"] == 1
        verify_event_chain(module, campaign.events_path)

        campaign.collect_sample = collect_sample

        mismatch = fake_node("validator-3")
        mismatch["validator_registry_sha256"] = "9" * 64
        campaign.audit_node = lambda node: (
            mismatch if node.name == "validator-3" else fake_node(node.name)
        )
        rejected = campaign.collect_sample()
        assert rejected["ok"] is False
        assert "validator validator_registry_sha256 values differ" in rejected["errors"]
        campaign.apply_sample(rejected)
        status = campaign.public_status()
        assert status["failures"]["total"] >= 1
        assert any(
            "validator validator_registry_sha256 values differ" in " ".join(item["errors"])
            for item in status["failures"]["recent"]
        )

        with campaign.lock_path.open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            try:
                with campaign.exclusive_lock():
                    raise AssertionError("campaign accepted a concurrent mutating command")
            except RuntimeError as error:
                assert "another campaign controller" in str(error)
            finally:
                fcntl.flock(lock, fcntl.LOCK_UN)

        campaign.audit_node = lambda node: {
            **fake_node(node.name),
            "active_alerts": ["PowerHouseValidatorDown"] if node.name == "validator-1" else [],
        }
        alerted = campaign.collect_sample()
        assert alerted["ok"] is False
        assert "active Prometheus alerts: PowerHouseValidatorDown" in alerted["errors"]

        campaign.audit_node = lambda node: {
            **fake_node(node.name),
            "active_alerts": (
                ["PowerHouseObserverIntakeUnavailable"] if node.name == "validator-1" else []
            ),
        }
        healthy_http = campaign._http_json

        def intake_down_http(url, data=None, timeout=8):
            if data is None and url.endswith("observer-intake-healthz"):
                raise RuntimeError("HTTP Error 502: Bad Gateway")
            return healthy_http(url, data=data, timeout=timeout)

        campaign._http_json = intake_down_http
        intake_incident = campaign.collect_sample()
        assert intake_incident["ok"] is False
        assert module.is_observer_intake_incident(intake_incident) is True
        before_failed = campaign.state["failed_samples"]
        before_successful = campaign.state["successful_samples"]
        campaign.apply_sample(intake_incident)
        status = campaign.public_status()
        assert campaign.state["failed_samples"] == before_failed
        assert campaign.state["successful_samples"] == before_successful + 1
        assert status["admission_plane"]["observer_intake_incidents"] == 1
        assert status["failures"]["observer_intake_total"] == 1
        assert status["failures"]["total"] == campaign.network_failed_samples()
        assert any(
            item["kind"] == "observer_intake_incident"
            for item in status["failures"]["recent"]
        )
        campaign._http_json = healthy_http

        campaign.state["last_sample_unix"] -= 20
        campaign.state["last_controller_event_unix"] -= 20
        before_failed = campaign.state["failed_samples"]
        before_samples = campaign.state["sample_count"]
        expected_missed = 20 // config.sample_interval_seconds - 1
        campaign.audit_node = lambda node: fake_node(node.name)
        campaign.apply_sample(campaign.collect_sample())
        assert campaign.state["failed_samples"] == before_failed
        assert campaign.state["sample_count"] == before_samples + 1
        assert campaign.state["missed_controller_samples"] == expected_missed
        assert json.loads(campaign.events_path.read_text().splitlines()[-2])["kind"] == "telemetry_gap"
        assert any(
            item["kind"] == "telemetry_gap"
            for item in campaign.public_status()["failures"]["recent"]
        )

        campaign.finalize()
        assert campaign.state["status"] == "failed"
        assert campaign.report_path.exists()
        assert campaign.manifest_path.exists()
        assert campaign.state["final_report_sha256"]
        verify_event_chain(module, campaign.events_path)

        campaign.state["drills"][0]["status"] = "running"
        module.atomic_json(campaign.state_path, campaign.state)
        resumed = module.Campaign(config)
        assert resumed.state["drills"][0]["status"] == "failed"
        assert "restarted during drill" in resumed.state["drills"][0]["detail"]

        gap_only_value = config_value(base / "gap-only")
        gap_only_path = base / "gap-only.json"
        gap_only_path.write_text(json.dumps(gap_only_value))
        gap_only = module.Campaign(module.Config.load(gap_only_path))
        install_fakes(gap_only)
        gap_only.apply_sample(gap_only.collect_sample())
        gap_only.state["last_sample_unix"] -= 20
        gap_only.state["last_controller_event_unix"] -= 20
        gap_only.apply_sample(gap_only.collect_sample())
        gap_only.state["drills"][0]["status"] = "passed"
        status = gap_only.public_status()
        assert status["sample_count"] == 2
        assert status["successful_samples"] == 2
        assert status["failed_samples"] == 0
        assert status["uptime_percent"] == 100.0
        assert status["controller_telemetry_gaps"]["missed_samples"] == expected_missed
        assert status["failures"]["controller_gap_total"] == expected_missed
        gap_only.finalize()
        assert gap_only.state["status"] == "passed"

        busy_value = config_value(base / "busy-gap")
        busy_path = base / "busy-gap.json"
        busy_path.write_text(json.dumps(busy_value))
        busy = module.Campaign(module.Config.load(busy_path))
        install_fakes(busy)
        busy.apply_sample(busy.collect_sample())
        busy.state["last_sample_unix"] -= 20
        busy.state["last_controller_event_unix"] = busy.state["last_sample_unix"]
        busy.record_event("rpc_burst", {"requests": 30, "errors": 0})
        busy.state["last_controller_event_unix"] = busy.state["last_sample_unix"]
        busy.apply_sample(busy.collect_sample())
        assert busy.state["missed_controller_samples"] == expected_missed
        result = busy.reconcile_controller_gaps()
        assert result["reclassified"] == 1
        status = busy.public_status()
        assert status["controller_telemetry_gaps"]["missed_samples"] == 0
        assert status["failures"]["controller_busy_windows"] == 1
        assert any(
            item["kind"] == "controller_busy_window"
            for item in status["failures"]["recent"]
        )

        finality_value = config_value(base / "finality-window")
        finality_path = base / "finality-window.json"
        finality_path.write_text(json.dumps(finality_value))
        finality = module.Campaign(module.Config.load(finality_path))
        install_fakes(finality)
        before = finality.collect_sample()
        finality.apply_sample(before)
        skew = json.loads(json.dumps(before))
        skew["ok"] = False
        skew["errors"] = ["finalized state differs across validators"]
        skew["nodes"][2]["health"]["finalized_block"] = 43
        skew["nodes"][2]["health"]["finalized_hash"] = "0x" + "6" * 64
        finality.apply_sample(skew)
        after = finality.collect_sample()
        finality.apply_sample(after)
        status = finality.public_status()
        assert status["sample_count"] == 3
        assert status["failed_samples"] == 1
        assert status["uptime_percent"] == round(2 / 3 * 100, 5)
        result = finality.reconcile_finality_windows()
        assert result["reclassified"] == 1
        assert result["network_failed_samples"] == 0
        status = finality.public_status()
        assert status["failed_samples"] == 0
        assert status["successful_samples"] == status["sample_count"]
        assert status["uptime_percent"] == 100.0
        assert status["consensus_plane"]["finality_convergence_windows"] == 1
        assert status["consensus_plane"]["finality_convergence_counted_as_failed_samples"] == 1
        assert status["failures"]["finality_convergence_total"] == 1
        assert any(
            item["kind"] == "finality_convergence_window"
            and item["reclassified_from"] == "sample"
            for item in status["failures"]["recent"]
        )

        legacy_value = config_value(base / "legacy-gap")
        legacy_path = base / "legacy-gap.json"
        legacy_path.write_text(json.dumps(legacy_value))
        legacy = module.Campaign(module.Config.load(legacy_path))
        install_fakes(legacy)
        legacy.apply_sample(legacy.collect_sample())
        legacy.state["sample_count"] += 1
        legacy.state["failed_samples"] += 1
        legacy.state["controller_gap_count"] = 1
        legacy.state["missed_controller_samples"] = 1
        legacy.state["max_controller_gap_seconds"] = 121.6
        legacy.state["controller_gaps_counted_as_failed_samples"] = 1
        legacy.state["drills"][0]["status"] = "passed"
        legacy.state["phase"] = "complete"
        legacy.state["status"] = "failed"
        legacy.state["rpc_latencies_ms"] = [100.0]
        legacy.record_failure(
            "telemetry_gap",
            {"gap_seconds": 121.6, "missed_samples": 1, "errors": []},
        )
        result = legacy.reclassify_controller_gap_outcome()
        assert result["status"] == "passed"
        assert result["network_failed_samples"] == 0
        assert legacy.state["status"] == "passed"
        status = legacy.public_status()
        assert status["sample_count"] == 1
        assert status["failed_samples"] == 0
        assert status["uptime_percent"] == 100.0
        assert status["evidence"]["final_report_sha256"]

        changed = config_value(base)
        changed["sample_interval_seconds"] = 6
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

        threshold = config_value(base / "threshold")
        threshold["max_rpc_p95_ms"] = 10
        threshold_path = base / "threshold.json"
        threshold_path.write_text(json.dumps(threshold))
        try:
            module.Config.load(threshold_path)
        except ValueError as error:
            assert "p95 threshold" in str(error)
        else:
            raise AssertionError("unsafe RPC latency threshold was accepted")

        for name, rpc_errors, latency, expected in (
            ("passing", 0, 100.0, "passed"),
            ("rpc-errors", 1, 100.0, "failed"),
            ("rpc-latency", 0, 1001.0, "failed"),
        ):
            gate_value = config_value(base / name)
            gate_path = base / f"{name}.json"
            gate_path.write_text(json.dumps(gate_value))
            gated = module.Campaign(module.Config.load(gate_path))
            install_fakes(gated)
            gated.apply_sample(gated.collect_sample())
            gated.state["drills"][0]["status"] = "passed"
            gated.state["rpc_errors"] = rpc_errors
            gated.state["rpc_latencies_ms"] = [latency]
            gated.finalize()
            assert gated.state["status"] == expected

    html = (ROOT / "publicpower" / "campaign.html").read_text()
    javascript = (ROOT / "publicpower" / "campaign.js").read_text()
    main_html = (ROOT / "publicpower" / "index.html").read_text()
    main_js = (ROOT / "publicpower" / "app.js").read_text()
    assert 'id="campaign-state"' in html
    assert 'id="drill-list"' in html
    assert 'id="failure-list"' in html
    assert 'id="acceptance-state"' in html
    assert 'id="campaign-note-title"' in html
    assert "reliability_campaign" in javascript
    assert "renderFailures" in javascript
    assert "NETWORK ON TRACK / ADMISSION AND EVIDENCE CAUTION" in javascript
    assert "FINALITY CONVERGENCE WINDOW" in javascript
    assert "reliability_campaign" not in main_js
    assert "campaign.html" in main_html
    deploy = (ROOT / "scripts" / "deploy_monitoring_stack.sh").read_text()
    assert "chmod 0755 /opt/powerhouse" in deploy
    assert "find /opt/powerhouse/releases" in deploy
    unit = (ROOT / "infra" / "systemd" / "powerhouse-reliability-campaign.service").read_text()
    assert "systemd-inhibit" in unit
    assert "ProtectSystem=strict" in unit
    assert "ProtectHome=read-only" in unit
    assert "ReadWritePaths=%h/.local/state/powerhouse/reliability" in unit
    controller_unit = (
        ROOT / "infra" / "systemd" / "powerhouse-reliability-controller.service"
    ).read_text()
    assert "User=powerhouse-campaign" in controller_unit
    assert "StateDirectory=powerhouse-reliability-controller" in controller_unit
    assert "ProtectSystem=strict" in controller_unit
    assert "Restart=on-failure" in controller_unit
    subprocess.run(["node", "--check", str(ROOT / "publicpower" / "campaign.js")], check=True)
    print("test_reliability_campaign: PASS")


if __name__ == "__main__":
    main()
