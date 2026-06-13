#!/usr/bin/env python3
"""Bounded, read-only JSON-RPC load test for the MFENX public edge."""

from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import json
import math
import threading
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


METHODS = ("eth_chainId", "eth_blockNumber", "web3_clientVersion")


def percentile(values: list[float], quantile: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = min(len(ordered) - 1, math.ceil(quantile * len(ordered)) - 1)
    return ordered[max(index, 0)]


def worker(url: str, deadline: float, latencies: list[float], errors: Counter, lock):
    sequence = 0
    while time.monotonic() < deadline:
        method = METHODS[sequence % len(METHODS)]
        sequence += 1
        payload = json.dumps(
            {"jsonrpc": "2.0", "id": sequence, "method": method, "params": []}
        ).encode()
        request = Request(
            url,
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        started = time.monotonic()
        try:
            with urlopen(request, timeout=8) as response:
                body = json.loads(response.read())
                if response.status != 200 or "result" not in body:
                    raise ValueError("invalid JSON-RPC response")
            elapsed = (time.monotonic() - started) * 1000
            with lock:
                latencies.append(elapsed)
        except (HTTPError, URLError, TimeoutError, ValueError, json.JSONDecodeError) as exc:
            key = (
                f"HTTP {exc.code}"
                if isinstance(exc, HTTPError)
                else type(exc).__name__
            )
            with lock:
                errors[key] += 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("url")
    parser.add_argument("--duration", type=int, default=30)
    parser.add_argument("--concurrency", type=int, default=10)
    parser.add_argument("--max-error-rate", type=float, default=0.01)
    parser.add_argument("--max-p95-ms", type=float, default=1000)
    args = parser.parse_args()
    if args.duration <= 0 or args.concurrency <= 0:
        parser.error("duration and concurrency must be positive")

    latencies: list[float] = []
    errors: Counter[str] = Counter()
    lock = threading.Lock()
    started = time.monotonic()
    deadline = started + args.duration
    with ThreadPoolExecutor(max_workers=args.concurrency) as executor:
        futures = [
            executor.submit(worker, args.url, deadline, latencies, errors, lock)
            for _ in range(args.concurrency)
        ]
        for future in futures:
            future.result()

    elapsed = time.monotonic() - started
    failed = sum(errors.values())
    total = len(latencies) + failed
    error_rate = failed / total if total else 1.0
    report = {
        "concurrency": args.concurrency,
        "duration_seconds": round(elapsed, 3),
        "error_rate": round(error_rate, 6),
        "errors": dict(errors),
        "latency_ms": {
            "p50": round(percentile(latencies, 0.50), 3),
            "p95": round(percentile(latencies, 0.95), 3),
            "p99": round(percentile(latencies, 0.99), 3),
        },
        "requests": total,
        "requests_per_second": round(total / elapsed, 3),
        "successful": len(latencies),
        "url": args.url,
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    if error_rate > args.max_error_rate:
        return 1
    if report["latency_ms"]["p95"] > args.max_p95_ms:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
