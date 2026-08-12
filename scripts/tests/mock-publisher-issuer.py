#!/usr/bin/env python3
"""Deterministic local HTTPS endpoint for publisher transport tests."""

from __future__ import annotations

import argparse
import http.server
from pathlib import Path
import signal
import ssl
import threading
import time


JWKS = b'{"keys":[]}\n'


class Server(http.server.ThreadingHTTPServer):
    mode: str
    count_file: Path | None
    request_count: int
    count_lock: threading.Lock


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        with self.server.count_lock:
            self.server.request_count += 1
            if self.server.count_file is not None:
                self.server.count_file.write_text(
                    f"{self.server.request_count}\n", encoding="ascii"
                )
        mode = self.server.mode
        if mode == "redirect":
            self.send_response(302)
            self.send_header("Location", f"https://localhost:{self.server.server_port}/jwks")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if mode == "slow":
            time.sleep(2)
        if mode == "oversize":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(256 * 1024 + 1))
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(JWKS)))
        self.end_headers()
        self.wfile.write(JWKS)

    def log_message(self, _format: str, *_arguments: object) -> None:
        pass


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cert", type=Path, required=True)
    parser.add_argument("--key", type=Path, required=True)
    parser.add_argument("--port-file", type=Path, required=True)
    parser.add_argument("--count-file", type=Path)
    parser.add_argument(
        "--mode", choices=("jwks", "redirect", "slow", "oversize"), required=True
    )
    args = parser.parse_args()

    server = Server(("127.0.0.1", 0), Handler)
    server.mode = args.mode
    server.count_file = args.count_file
    server.request_count = 0
    server.count_lock = threading.Lock()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(args.cert, args.key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    args.port_file.write_text(f"{server.server_port}\n", encoding="ascii")

    signal.signal(signal.SIGTERM, lambda _signum, _frame: server.shutdown())
    server.serve_forever()


if __name__ == "__main__":
    main()
