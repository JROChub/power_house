#!/usr/bin/env python3
"""Minimal loopback HTTPS server for browser integration tests."""

from __future__ import annotations

import argparse
import os
import ssl
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cert", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--key", type=Path, required=True)
    parser.add_argument("--port", type=int, default=8766)
    arguments = parser.parse_args()
    if not 1 <= arguments.port <= 65535:
        raise ValueError("port is outside the TCP range")
    directory = arguments.directory.resolve(strict=True)
    if not directory.is_dir():
        raise ValueError("served path is not a directory")
    os.chdir(directory)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(arguments.cert, arguments.key)
    server = ThreadingHTTPServer(("127.0.0.1", arguments.port), SimpleHTTPRequestHandler)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
