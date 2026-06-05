#!/usr/bin/env python3
"""Validate a public EVM JSON-RPC endpoint before wallet publication."""

from __future__ import annotations

import argparse
import json
import re
import socket
import sys
import urllib.error
import urllib.parse
import urllib.request
from typing import Any


HASH_RE = re.compile(r"^0x[0-9a-fA-F]{64}$")
QUANTITY_RE = re.compile(r"^0x(?:0|[1-9a-fA-F][0-9a-fA-F]*)$")
TEST_ORIGIN = "https://mfenx.com"


class RpcCheckError(RuntimeError):
    pass


def resolve_endpoint(url: str) -> tuple[urllib.parse.ParseResult, list[str]]:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme not in {"http", "https"} or not parsed.hostname:
        raise RpcCheckError("RPC URL must use http:// or https:// and include a hostname")
    port = parsed.port or (443 if parsed.scheme == "https" else 80)
    try:
        records = socket.getaddrinfo(parsed.hostname, port, type=socket.SOCK_STREAM)
    except socket.gaierror as exc:
        raise RpcCheckError(f"DNS lookup failed for {parsed.hostname}: {exc}") from exc
    addresses = sorted({record[4][0] for record in records})
    if not addresses:
        raise RpcCheckError(f"DNS lookup returned no addresses for {parsed.hostname}")
    return parsed, addresses


def rpc_call(
    url: str,
    method: str,
    params: list[Any],
    request_id: int,
    timeout: float,
) -> tuple[Any, Any]:
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
    ).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=payload,
        headers={
            "Accept": "application/json",
            "Content-Type": "application/json",
            "Origin": TEST_ORIGIN,
            "User-Agent": "power-house-rpc-check/1",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read()
            headers = response.headers
    except urllib.error.HTTPError as exc:
        detail = exc.read(512).decode("utf-8", errors="replace")
        raise RpcCheckError(f"{method}: HTTP {exc.code}: {detail}") from exc
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise RpcCheckError(f"{method}: transport failure: {exc}") from exc
    try:
        document = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RpcCheckError(f"{method}: response is not valid JSON") from exc
    if not isinstance(document, dict):
        raise RpcCheckError(f"{method}: response must be a JSON object")
    if document.get("jsonrpc") != "2.0":
        raise RpcCheckError(f"{method}: missing JSON-RPC 2.0 marker")
    if document.get("id") != request_id:
        raise RpcCheckError(f"{method}: response ID does not match request")
    if document.get("error") is not None:
        raise RpcCheckError(f"{method}: RPC error: {document['error']}")
    if "result" not in document:
        raise RpcCheckError(f"{method}: response has no result")
    return document["result"], headers


def parse_quantity(value: Any, label: str) -> int:
    if not isinstance(value, str) or not QUANTITY_RE.fullmatch(value):
        raise RpcCheckError(f"{label}: expected a canonical hexadecimal quantity")
    return int(value, 16)


def parse_network_id(value: Any) -> int:
    if not isinstance(value, str) or not value.isdecimal():
        raise RpcCheckError("net_version: expected a decimal network ID string")
    return int(value, 10)


def require_hash(value: Any, label: str) -> None:
    if not isinstance(value, str) or not HASH_RE.fullmatch(value):
        raise RpcCheckError(f"{label}: expected a 32-byte hexadecimal hash")


def run_check(
    url: str,
    *,
    expected_chain_id: int,
    timeout: float,
    require_cors: bool,
) -> dict[str, Any]:
    parsed, addresses = resolve_endpoint(url)
    client, first_headers = rpc_call(url, "web3_clientVersion", [], 1, timeout)
    if not isinstance(client, str) or not client.strip():
        raise RpcCheckError("web3_clientVersion: expected a non-empty string")

    if require_cors:
        allow_origin = first_headers.get("Access-Control-Allow-Origin", "")
        if allow_origin not in {"*", TEST_ORIGIN}:
            raise RpcCheckError(
                "browser CORS is not enabled for the Power-House site origin"
            )

    chain_id = parse_quantity(
        rpc_call(url, "eth_chainId", [], 2, timeout)[0], "eth_chainId"
    )
    network_id = parse_network_id(rpc_call(url, "net_version", [], 3, timeout)[0])
    if chain_id != expected_chain_id:
        raise RpcCheckError(
            f"eth_chainId mismatch: expected {expected_chain_id}, received {chain_id}"
        )
    if network_id != expected_chain_id:
        raise RpcCheckError(
            f"net_version mismatch: expected {expected_chain_id}, received {network_id}"
        )

    block_before = parse_quantity(
        rpc_call(url, "eth_blockNumber", [], 4, timeout)[0], "eth_blockNumber"
    )
    block = rpc_call(url, "eth_getBlockByNumber", ["latest", False], 5, timeout)[0]
    if not isinstance(block, dict):
        raise RpcCheckError("eth_getBlockByNumber: latest block must be an object")
    block_number = parse_quantity(block.get("number"), "latest block number")
    require_hash(block.get("hash"), "latest block hash")
    require_hash(block.get("parentHash"), "latest parent hash")
    parse_quantity(block.get("timestamp"), "latest block timestamp")
    block_after = parse_quantity(
        rpc_call(url, "eth_blockNumber", [], 6, timeout)[0], "eth_blockNumber"
    )
    if block_after < block_before:
        raise RpcCheckError("eth_blockNumber regressed during the probe")
    if not block_before <= block_number <= block_after:
        raise RpcCheckError(
            "latest block response is inconsistent with eth_blockNumber"
        )

    return {
        "url": url,
        "host": parsed.hostname,
        "addresses": addresses,
        "client": client,
        "chain_id": chain_id,
        "network_id": network_id,
        "block_number": block_after,
        "block_hash": block["hash"],
        "cors": first_headers.get("Access-Control-Allow-Origin"),
    }


def parser() -> argparse.ArgumentParser:
    cli = argparse.ArgumentParser(
        description="Validate DNS, JSON-RPC identity, latest-block integrity, and CORS"
    )
    cli.add_argument("url", help="public HTTP(S) JSON-RPC URL")
    cli.add_argument("--expected-chain-id", type=int, default=177155)
    cli.add_argument("--timeout", type=float, default=10.0)
    cli.add_argument("--require-cors", action="store_true")
    cli.add_argument("--json", action="store_true", dest="json_output")
    return cli


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = run_check(
            args.url,
            expected_chain_id=args.expected_chain_id,
            timeout=args.timeout,
            require_cors=args.require_cors,
        )
    except RpcCheckError as exc:
        print(f"RPC CHECK FAILED: {exc}", file=sys.stderr)
        return 1

    if args.json_output:
        print(json.dumps(result, sort_keys=True))
    else:
        print(f"RPC CHECK PASS: {result['url']}")
        print(f"  addresses: {', '.join(result['addresses'])}")
        print(f"  client: {result['client']}")
        print(f"  chain: {result['chain_id']}")
        print(f"  latest block: {result['block_number']} ({result['block_hash']})")
        if args.require_cors:
            print(f"  cors: {result['cors']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
