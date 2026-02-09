#!/usr/bin/env python3
import json
import os
import subprocess
import sys
import time
import urllib.request

SERVICE = os.environ.get("PH_SERVICE_NAME", "powerhouse-boot")
HEALTH_URL = os.environ.get("PH_HEALTH_URL", "http://127.0.0.1:8181/healthz")
METRICS_URL = os.environ.get("PH_METRICS_URL", "http://127.0.0.1:9100/metrics")
AUTH_TOKEN = os.environ.get("PH_BLOB_AUTH_TOKEN", "")
STATE_PATH = os.environ.get("PH_HEALTH_STATE", "/var/lib/powerhouse/ops/health_state.json")
STALL_MINUTES = int(os.environ.get("PH_METRICS_STALL_MINUTES", "20"))

ALERT_SCRIPT = os.environ.get("PH_ALERT_SCRIPT", "/usr/local/lib/powerhouse/alert.sh")

errors = []
warnings = []


def call_systemctl(service):
    try:
        res = subprocess.run(
            ["systemctl", "is-active", service],
            capture_output=True,
            text=True,
            check=False,
        )
        return res.stdout.strip()
    except Exception as exc:
        errors.append(f"systemctl failed: {exc}")
        return "unknown"


def http_get(url, auth_token=None):
    req = urllib.request.Request(url)
    if auth_token:
        req.add_header("Authorization", f"Bearer {auth_token}")
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            return resp.status, resp.read().decode("utf-8", "replace")
    except Exception as exc:
        return 0, str(exc)


def parse_metrics(body):
    metrics = {}
    for line in body.splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2:
            continue
        key, val = parts
        try:
            metrics[key] = float(val)
        except ValueError:
            continue
    return metrics


status = call_systemctl(SERVICE)
if status != "active":
    errors.append(f"service {SERVICE} not active (state={status})")

health_status, health_body = http_get(HEALTH_URL, AUTH_TOKEN or None)
if health_status != 200:
    errors.append(f"healthz failed ({health_status}): {health_body}")

metrics_status, metrics_body = http_get(METRICS_URL)
if metrics_status != 200:
    errors.append(f"metrics failed ({metrics_status}): {metrics_body}")
    metrics = {}
else:
    metrics = parse_metrics(metrics_body)

required = [
    "anchors_received_total",
    "anchors_verified_total",
    "finality_events_total",
]
for key in required:
    if key not in metrics:
        warnings.append(f"metrics missing {key}")

now = int(time.time())

state = {}
try:
    with open(STATE_PATH, "r", encoding="utf-8") as fh:
        state = json.load(fh)
except FileNotFoundError:
    state = {}
except Exception as exc:
    warnings.append(f"failed to read state: {exc}")

last = state.get("last", {})
last_seen = state.get("last_seen", now)

if metrics:
    current_finality = metrics.get("finality_events_total", 0.0)
    last_finality = last.get("finality_events_total", current_finality)
    if current_finality <= last_finality:
        elapsed = now - last_seen
        if elapsed > STALL_MINUTES * 60:
            warnings.append(
                f"finality stalled ({elapsed // 60}m without increment)"
            )
    else:
        last_seen = now

    state = {
        "last": {
            "finality_events_total": current_finality,
            "anchors_received_total": metrics.get("anchors_received_total", 0.0),
            "anchors_verified_total": metrics.get("anchors_verified_total", 0.0),
        },
        "last_seen": last_seen,
        "updated": now,
    }
    os.makedirs(os.path.dirname(STATE_PATH), exist_ok=True)
    with open(STATE_PATH, "w", encoding="utf-8") as fh:
        json.dump(state, fh)

exit_code = 0
if warnings:
    exit_code = 1
if errors:
    exit_code = 2

if errors or warnings:
    detail = "\n".join(["Errors:"] + errors + ["", "Warnings:"] + warnings)
    try:
        subprocess.run([ALERT_SCRIPT, "Power-House healthcheck", detail], check=False)
    except Exception:
        pass

if errors:
    for msg in errors:
        print(msg)
if warnings:
    for msg in warnings:
        print(msg)

sys.exit(exit_code)
