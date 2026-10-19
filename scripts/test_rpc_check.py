#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


SCRIPT = Path(__file__).with_name("check_rpc.py")
SPEC = importlib.util.spec_from_file_location("check_rpc", SCRIPT)
assert SPEC and SPEC.loader
check_rpc = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_rpc)


class RpcHandler(BaseHTTPRequestHandler):
    chain_id = 177155
    malformed_hash = False
    syncing = False
    bad_rewards = False
    genesis_hash = "0x" + "12" * 32
    boolean_id = False
    oversized = False

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        request = json.loads(self.rfile.read(length))
        method = request["method"]
        results = {
            "web3_clientVersion": "Power-House/test",
            "eth_chainId": hex(self.chain_id),
            "net_version": str(self.chain_id),
            "eth_blockNumber": "0x2a",
            "eth_syncing": self.syncing,
            "eth_getBalance": "0x0",
            "eth_getTransactionCount": "0x0",
            "eth_gasPrice": "0x0",
            "eth_maxPriorityFeePerGas": "0x0",
            "eth_feeHistory": {
                "oldestBlock": "0x2a", "baseFeePerGas": ["0x0", "0x0"],
                "gasUsedRatio": [0.0], "reward": [] if self.bad_rewards else [["0x0"]],
            },
            "eth_getBlockByNumber": {
                "number": "0x2a",
                "hash": "0x1234"
                if self.malformed_hash
                else "0x" + ("ab" * 32),
                "parentHash": "0x" + ("cd" * 32),
                "timestamp": "0x65f00000",
            },
        }
        if method == "eth_getBlockByNumber" and request["params"][0] == "0x0":
            results[method] = {"number": "0x0", "hash": self.genesis_hash}
        if self.oversized:
            results[method] = "x" * (check_rpc.MAX_RESPONSE_BYTES + 1)
        response = json.dumps(
            {"jsonrpc": "2.0", "id": True if self.boolean_id else request["id"], "result": results[method]}
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "https://mfenx.com")
        self.send_header("Content-Length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)

    def log_message(self, _format: str, *_args: object) -> None:
        return


class RpcCheckTests(unittest.TestCase):
    def run_server(self, handler: type[RpcHandler]) -> tuple[ThreadingHTTPServer, str]:
        server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return server, f"http://127.0.0.1:{server.server_port}"

    def test_valid_endpoint(self) -> None:
        server, url = self.run_server(RpcHandler)
        result = check_rpc.run_check(
            url, expected_chain_id=177155, timeout=2, require_cors=True
        )
        self.assertEqual(result["chain_id"], 177155)
        self.assertEqual(result["block_number"], 42)

    def test_chain_id_mismatch_fails(self) -> None:
        class WrongChain(RpcHandler):
            chain_id = 1

        server, url = self.run_server(WrongChain)
        with self.assertRaisesRegex(check_rpc.RpcCheckError, "mismatch"):
            check_rpc.run_check(
                url, expected_chain_id=177155, timeout=2, require_cors=False
            )

    def test_malformed_block_hash_fails(self) -> None:
        class BadHash(RpcHandler):
            malformed_hash = True

        server, url = self.run_server(BadHash)
        with self.assertRaisesRegex(check_rpc.RpcCheckError, "block hash"):
            check_rpc.run_check(
                url, expected_chain_id=177155, timeout=2, require_cors=False
            )

    def test_genesis_and_wallet_methods(self) -> None:
        _, url = self.run_server(RpcHandler)
        result = check_rpc.run_check(url, expected_chain_id=177155, timeout=2, require_cors=True,
                                     expected_genesis=RpcHandler.genesis_hash, wallet_checks=True)
        self.assertEqual(result["genesis_hash"], RpcHandler.genesis_hash)
        self.assertIn("eth_feeHistory", result["wallet_methods_checked"])

    def test_different_history_with_same_chain_id_is_rejected(self) -> None:
        _, url = self.run_server(RpcHandler)
        with self.assertRaisesRegex(check_rpc.RpcCheckError, "genesis mismatch"):
            check_rpc.run_check(url, expected_chain_id=177155, timeout=2, require_cors=True,
                                expected_genesis="0x" + "34" * 32)

    def test_wallet_failures_are_rejected(self) -> None:
        for attribute, value, message in [("syncing", {}, "not ready"), ("bad_rewards", True, "reward dimensions")]:
            with self.subTest(attribute=attribute):
                handler = type("WalletFailure", (RpcHandler,), {attribute: value})
                _, url = self.run_server(handler)
                with self.assertRaisesRegex(check_rpc.RpcCheckError, message):
                    check_rpc.run_check(url, expected_chain_id=177155, timeout=2, require_cors=True, wallet_checks=True)

    def test_response_bounds_and_strict_ids(self) -> None:
        for attribute, message in [("boolean_id", "ID does not match"), ("oversized", "byte limit")]:
            with self.subTest(attribute=attribute):
                handler = type("Malformed", (RpcHandler,), {attribute: True})
                _, url = self.run_server(handler)
                with self.assertRaisesRegex(check_rpc.RpcCheckError, message):
                    check_rpc.run_check(url, expected_chain_id=177155, timeout=2, require_cors=False)


if __name__ == "__main__":
    unittest.main()
