#!/usr/bin/env python3
import json
import os
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import Request, urlopen


PROMETHEUS_URL = os.environ.get("PROMETHEUS_URL", "http://127.0.0.1:9090")
RPC_URL = os.environ.get("RPC_URL", "https://rpc.mfenx.com")
RELEASE = os.environ.get("POWER_HOUSE_RELEASE", "unknown")
REGISTRY_STATE_PATH = os.environ.get(
    "VALIDATOR_REGISTRY_STATE",
    "/var/lib/powerhouse/monitoring/validator-registry-state.json",
)
REGISTRY_MAX_AGE_SECONDS = int(os.environ.get("VALIDATOR_REGISTRY_MAX_AGE", "45"))
OBSERVER_STATE_PATH = os.environ.get(
    "OBSERVER_REGISTRY_STATE",
    "/var/lib/powerhouse/monitoring/observer-registry-state.json",
)
OBSERVER_MAX_AGE_SECONDS = int(os.environ.get("OBSERVER_REGISTRY_MAX_AGE", "45"))


def fetch_json(url, data=None, headers=None):
    request = Request(url, data=data, headers=headers or {})
    with urlopen(request, timeout=5) as response:
        return json.loads(response.read())


def query(expression):
    from urllib.parse import urlencode

    payload = fetch_json(
        f"{PROMETHEUS_URL}/api/v1/query?{urlencode({'query': expression})}"
    )
    results = payload.get("data", {}).get("result", [])
    if not results:
        return None
    return float(results[0]["value"][1])


def rpc(method):
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": []}
    ).encode()
    response = fetch_json(
        RPC_URL,
        data=payload,
        headers={"Content-Type": "application/json"},
    )
    return response["result"]


def registry_health():
    with open(REGISTRY_STATE_PATH, encoding="utf-8") as handle:
        state = json.load(handle)
    generated = datetime.fromisoformat(state["generated_at"].replace("Z", "+00:00"))
    age = (datetime.now(timezone.utc) - generated).total_seconds()
    state["fresh"] = 0 <= age <= REGISTRY_MAX_AGE_SECONDS
    return state


def observer_health():
    try:
        with open(OBSERVER_STATE_PATH, encoding="utf-8") as handle:
            state = json.load(handle)
    except FileNotFoundError:
        return {
            "configured": False,
            "fresh": False,
            "registry_verified": False,
            "observers_total": 0,
            "observers_healthy": 0,
            "observer_connections": 0,
            "observers": [],
        }
    generated = datetime.fromisoformat(state["generated_at"].replace("Z", "+00:00"))
    age = (datetime.now(timezone.utc) - generated).total_seconds()
    state["fresh"] = (
        state.get("configured") is True
        and state.get("registry_verified") is True
        and 0 <= age <= OBSERVER_MAX_AGE_SECONDS
    )
    return state


def snapshot():
    registry = registry_health()
    observer = observer_health()
    validators = int(registry.get("validators_healthy", 0))
    validators_total = int(registry.get("validators_total", 0))
    systems = sum(
        1
        for item in registry.get("validators", [])
        if item.get("system_metrics_reachable") is True
    )
    rpc_probe = int(query('min(probe_success{job="blackbox-rpc"})') or 0)
    validator_links = int(registry.get("peer_link_observations", 0))
    observer_connections = int(observer.get("observer_connections", 0))
    uptime = query(
        'avg_over_time(probe_success{job="blackbox-rpc"}[24h])'
    )
    runtime = fetch_json(f"{PROMETHEUS_URL}/api/v1/status/runtimeinfo")
    started = runtime.get("data", {}).get("startTime")
    monitoring_age = 0.0
    if started:
        monitoring_age = (
            datetime.now(timezone.utc)
            - datetime.fromisoformat(started.replace("Z", "+00:00"))
        ).total_seconds()
    chain_id = int(rpc("eth_chainId"), 16)
    block_height = int(rpc("eth_blockNumber"), 16)
    client = rpc("web3_clientVersion")
    registry_ok = registry.get("registry_verified") is True and registry["fresh"]
    if (
        registry_ok
        and validators_total > 0
        and validators == validators_total
        and systems == validators_total
        and rpc_probe == 1
    ):
        state = "operational"
    elif registry_ok and validators > 0 and rpc_probe == 1:
        state = "degraded"
    else:
        state = "outage"
    return {
        "block_height": block_height,
        "chain_id": chain_id,
        "client": client,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "network": "MFENX Power House",
        "observer_peers": {
            "configured": observer.get("configured") is True,
            "connected": observer_connections,
            "fresh": observer.get("fresh") is True,
            "healthy": int(observer.get("observers_healthy", 0)),
            "total": int(observer.get("observers_total", 0)),
        },
        "observer_registry": {
            "configured": observer.get("configured") is True,
            "fresh": observer.get("fresh") is True,
            "identity_verified": sum(
                1
                for item in observer.get("observers", [])
                if item.get("identity_verified") is True
            ),
            "verified": observer.get("registry_verified") is True,
        },
        "peer_connections": validator_links,
        "public_peer_connections": observer_connections,
        "release": RELEASE,
        "rpc": {
            "endpoint": RPC_URL,
            "name": "LAX MFENX RPC",
            "reachable": rpc_probe == 1,
        },
        "status": state,
        "system_exporters_healthy": systems,
        "uptime_24h": (
            round(uptime * 100, 3)
            if uptime is not None and monitoring_age >= 86_400
            else None
        ),
        "validators_healthy": validators,
        "validators_total": validators_total,
        "validator_peer_links": validator_links,
        "validator_registry": {
            "fresh": registry["fresh"],
            "identity_verified": sum(
                1
                for item in registry.get("validators", [])
                if item.get("identity_verified") is True
            ),
            "verified": registry.get("registry_verified") is True,
        },
    }


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path not in {"/", "/status.json", "/healthz"}:
            self.send_error(404)
            return
        try:
            body = json.dumps(snapshot(), sort_keys=True).encode()
            status = 200
        except Exception as exc:
            body = json.dumps(
                {
                    "generated_at": datetime.now(timezone.utc).isoformat(),
                    "status": "outage",
                    "error": str(exc),
                },
                sort_keys=True,
            ).encode()
            status = 503
        self.send_response(status)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        return


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", 9194), Handler).serve_forever()
