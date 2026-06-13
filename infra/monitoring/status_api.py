#!/usr/bin/env python3
import json
import os
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import Request, urlopen


PROMETHEUS_URL = os.environ.get("PROMETHEUS_URL", "http://127.0.0.1:9090")
RPC_URL = os.environ.get("RPC_URL", "https://rpc.mfenx.com")
RELEASE = os.environ.get("POWER_HOUSE_RELEASE", "unknown")


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


def snapshot():
    validators = int(
        query(
            'count(count by (node) '
            '(up{job="powerhouse",node=~"validator-[123]"} == 1))'
        )
        or 0
    )
    systems = int(
        query(
            'count(count by (node) '
            '(up{job="node",node=~"validator-[123]"} == 1))'
        )
        or 0
    )
    rpc_probe = int(query('min(probe_success{job="blackbox-rpc"})') or 0)
    peers = int(
        query(
            'sum(max by (node) '
            '(powerhouse_connected_peers{'
            'job="powerhouse",node=~"validator-[123]"}))'
        )
        or 0
    )
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
    if validators == 3 and systems == 3 and rpc_probe == 1:
        state = "operational"
    elif validators >= 2 and rpc_probe == 1:
        state = "degraded"
    else:
        state = "outage"
    return {
        "block_height": block_height,
        "chain_id": chain_id,
        "client": client,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "network": "MFENX Power House",
        "peer_connections": peers,
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
        "validators_total": 3,
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
