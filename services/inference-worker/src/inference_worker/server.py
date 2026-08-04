from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, HTTPServer

from .config import load_config


class Handler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/health":
            payload = json.dumps({"status": "ok"}).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return

        self.send_response(404)
        self.end_headers()


def serve() -> None:
    config = load_config()
    server = HTTPServer((config.host, config.port), Handler)
    server.serve_forever()

