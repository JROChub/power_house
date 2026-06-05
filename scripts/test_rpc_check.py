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

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        request = json.loads(self.rfile.read(length))
        method = request["method"]
        results = {
            "web3_clientVersion": "Power-House/test",
            "eth_chainId": hex(self.chain_id),
            "net_version": str(self.chain_id),
            "eth_blockNumber": "0x2a",
            "eth_getBlockByNumber": {
                "number": "0x2a",
                "hash": "0x1234"
                if self.malformed_hash
                else "0x" + ("ab" * 32),
                "parentHash": "0x" + ("cd" * 32),
                "timestamp": "0x65f00000",
            },
        }
        response = json.dumps(
            {"jsonrpc": "2.0", "id": request["id"], "result": results[method]}
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


if __name__ == "__main__":
    unittest.main()
