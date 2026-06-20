#!/usr/bin/env python3
import json
import os
import ipaddress
import re
import socket
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse
from urllib.request import HTTPRedirectHandler, Request, build_opener, urlopen


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
CAMPAIGN_STATE_PATH = os.environ.get(
    "RELIABILITY_CAMPAIGN_STATE",
    "/var/lib/powerhouse/reliability/campaign-status.json",
)
CAMPAIGN_MAX_AGE_SECONDS = int(os.environ.get("RELIABILITY_CAMPAIGN_MAX_AGE", "180"))
MAX_PROBE_BYTES = 2 * 1024 * 1024
IDENTITY_METRIC = re.compile(
    r'^powerhouse_node_identity\{(?P<labels>.+)\}\s+(?P<value>[0-9.eE+-]+)$'
)
SIMPLE_METRIC = re.compile(
    r"^(?P<name>[a-zA-Z_:][a-zA-Z0-9_:]*)\s+(?P<value>[0-9.eE+-]+)$"
)
LABEL = re.compile(r'([a-zA-Z_][a-zA-Z0-9_]*)="((?:\\.|[^"])*)"')


class RejectRedirects(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError(f"redirect rejected: HTTP {code}")


def fetch_json(url, data=None, headers=None):
    request = Request(url, data=data, headers=headers or {})
    with urlopen(request, timeout=5) as response:
        return json.loads(response.read())


def decode_label(value):
    return (
        value.replace(r"\\", "\0")
        .replace(r"\"", '"')
        .replace(r"\n", "\n")
        .replace("\0", "\\")
    )


def parse_metrics(body):
    identity = None
    metrics = {}
    for line in body.splitlines():
        if not line or line.startswith("#"):
            continue
        match = IDENTITY_METRIC.fullmatch(line)
        if match:
            labels = {
                name: decode_label(value)
                for name, value in LABEL.findall(match.group("labels"))
            }
            if float(match.group("value")) == 1:
                identity = labels
            continue
        metric = SIMPLE_METRIC.fullmatch(line)
        if metric:
            metrics[metric.group("name")] = float(metric.group("value"))
    return identity, metrics


def public_addresses_for_host(host):
    if not host:
        raise ValueError("host is required")
    if "://" in host or "/" in host:
        raise ValueError("host must be a bare public DNS name or IP address")
    try:
        addresses = [ipaddress.ip_address(host.strip("[]"))]
    except ValueError:
        records = socket.getaddrinfo(host, None, type=socket.SOCK_STREAM)
        addresses = sorted({ipaddress.ip_address(record[4][0]) for record in records})
    if not addresses:
        raise ValueError("host did not resolve")
    for address in addresses:
        if not address.is_global:
            raise ValueError(f"refusing non-public target address {address}")
    return addresses


def parse_probe_port(raw, default):
    if raw is None:
        return default
    port = int(raw)
    if port < 1 or port > 65535:
        raise ValueError("port must be between 1 and 65535")
    return port


def probe_tcp(host, port, timeout):
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return {"reachable": True, "error": None}
    except Exception as exc:
        return {"reachable": False, "error": str(exc)}


def probe_metrics(host, port, timeout):
    target = f"http://{host}:{port}/metrics"
    if ":" in host and not host.startswith("["):
        target = f"http://[{host}]:{port}/metrics"
    request = Request(target, headers={"User-Agent": "power-house-observer-probe/1"})
    opener = build_opener(RejectRedirects)
    try:
        with opener.open(request, timeout=timeout) as response:
            body = response.read(MAX_PROBE_BYTES + 1)
            if len(body) > MAX_PROBE_BYTES:
                raise RuntimeError("metrics response exceeds maximum size")
        text = body.decode("utf-8", "replace")
        identity, metrics = parse_metrics(text)
        return {
            "reachable": True,
            "identity_found": identity is not None,
            "identity": identity,
            "connected_peers": int(metrics.get("powerhouse_connected_peers", 0)),
            "error": None,
        }
    except Exception as exc:
        return {
            "reachable": False,
            "identity_found": False,
            "identity": None,
            "connected_peers": 0,
            "error": str(exc),
        }


def observer_probe(query):
    values = parse_qs(query)
    host = values.get("host", [""])[0].strip()
    metrics_port = parse_probe_port(values.get("metrics_port", [None])[0], 9102)
    p2p_port = parse_probe_port(values.get("p2p_port", [None])[0], 7001)
    timeout = min(parse_probe_port(values.get("timeout", [None])[0], 5), 10)
    addresses = public_addresses_for_host(host)
    metrics = probe_metrics(host, metrics_port, timeout)
    p2p = probe_tcp(host, p2p_port, timeout)
    return {
        "schema": "power-house-observer-probe-v1",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "target": {
            "host": host,
            "resolved_addresses": [str(address) for address in addresses],
            "metrics_port": metrics_port,
            "p2p_port": p2p_port,
        },
        "metrics": metrics,
        "p2p": p2p,
        "ok": metrics.get("reachable") is True
        and metrics.get("identity_found") is True
        and p2p.get("reachable") is True,
    }


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


def campaign_health():
    try:
        with open(CAMPAIGN_STATE_PATH, encoding="utf-8") as handle:
            state = json.load(handle)
    except FileNotFoundError:
        return {
            "schema": "power-house-reliability-campaign-v1",
            "status": "not_started",
            "phase": "awaiting",
            "fresh": False,
        }
    if state.get("schema") != "power-house-reliability-campaign-v1":
        raise RuntimeError("reliability campaign schema mismatch")
    updated = datetime.fromisoformat(state["updated_at"].replace("Z", "+00:00"))
    age = (datetime.now(timezone.utc) - updated).total_seconds()
    state["fresh"] = 0 <= age <= CAMPAIGN_MAX_AGE_SECONDS
    if state.get("status") == "running" and not state["fresh"]:
        state["status"] = "stalled"
        state["phase"] = "telemetry_gap"
    return state


def snapshot():
    registry = registry_health()
    observer = observer_health()
    campaign = campaign_health()
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
        "reliability_campaign": campaign,
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
        parsed = urlparse(self.path)
        if parsed.path == "/observer-probe":
            try:
                body = json.dumps(observer_probe(parsed.query), sort_keys=True).encode()
                status = 200
            except Exception as exc:
                body = json.dumps(
                    {
                        "generated_at": datetime.now(timezone.utc).isoformat(),
                        "schema": "power-house-observer-probe-v1",
                        "ok": False,
                        "error": str(exc),
                    },
                    sort_keys=True,
                ).encode()
                status = 400
            self.send_response(status)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if parsed.path not in {"/", "/status.json", "/healthz"}:
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
