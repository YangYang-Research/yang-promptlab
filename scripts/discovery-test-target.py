#!/usr/bin/env python3
"""PromptLab discovery verification target — serves http://localhost:3000"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import json


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[target] {args[0]}")

    def do_GET(self):
        routes = {
            "/": (
                200,
                "text/html",
                """<!doctype html>
<html><body>
  <a href="/docs">Docs</a>
  <a href="/api/v1/users">Users API</a>
  <a href="/internal">Internal</a>
  <script>fetch("/api/v1/chat/completions");</script>
</body></html>""",
            ),
            "/docs": (200, "text/html", "<html><body>internal docs</body></html>"),
            "/api/v1/users": (
                200,
                "application/json",
                json.dumps({"data": []}),
            ),
            "/openapi.json": (
                200,
                "application/json",
                json.dumps(
                    {
                        "openapi": "3.1.0",
                        "info": {"title": "PromptLab Test", "version": "1.0.0"},
                        "paths": {},
                    }
                ),
            ),
            "/graphql": (200, "text/html", "<html>graphiql playground</html>"),
            "/v1/models": (
                200,
                "application/json",
                json.dumps({"data": [{"id": "gpt-4", "object": "model"}]}),
            ),
        }
        if self.path in routes:
            status, ctype, body = routes[self.path]
            self.send_response(status)
            self.send_header("Content-Type", ctype)
            self.end_headers()
            self.wfile.write(body.encode())
            return
        self.send_response(404)
        self.end_headers()

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        _ = self.rfile.read(length) if length else b""
        if self.path == "/graphql":
            body = json.dumps(
                {"data": {"__schema": {"queryType": {"name": "Query"}}}}
            )
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(body.encode())
            return
        if self.path == "/v1/chat/completions":
            body = json.dumps(
                {
                    "error": {
                        "message": "missing bearer token",
                        "type": "invalid_request_error",
                    }
                }
            )
            self.send_response(401)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(body.encode())
            return
        self.send_response(404)
        self.end_headers()


if __name__ == "__main__":
    host, port = "127.0.0.1", 3000
    print(f"Discovery test target listening on http://{host}:{port}")
    HTTPServer((host, port), Handler).serve_forever()
