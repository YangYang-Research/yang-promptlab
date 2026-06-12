#!/usr/bin/env python3
"""AISec SPA discovery target — serves http://localhost:3100

The landing page contains NO literal API URLs; the endpoints are built
dynamically in JavaScript at runtime, so only a real browser (Playwright)
that executes the SPA will observe the resulting network traffic.
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import json

INDEX_HTML = """<!doctype html>
<html>
  <head><title>AISec SPA Demo</title></head>
  <body>
    <div id="app">Loading...</div>
    <script>
      // URLs are assembled at runtime so they are invisible to static crawlers.
      const seg = (a, b) => a + '/' + b;
      async function boot() {
        const apiBase = seg(seg('', 'api'), 'v1');           // "/api/v1"
        await fetch(apiBase + '/profile');
        await fetch(apiBase + '/items', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: '{}'
        });
        const chat = seg(seg('', 'v1'), 'chat') + '/completions'; // "/v1/chat/completions"
        await fetch(chat, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: '{"model":"demo","messages":[]}'
        });
        document.getElementById('app').textContent = 'Loaded';
      }
      boot();
    </script>
  </body>
</html>"""


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[spa-target] {args[0]}")

    def _send(self, code, ctype, body):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/":
            self._send(200, "text/html", INDEX_HTML.encode())
        elif self.path == "/api/v1/profile":
            self._send(200, "application/json", json.dumps({"id": 1, "name": "demo"}).encode())
        else:
            self._send(404, "application/json", b'{"error":"not found"}')

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        if length:
            self.rfile.read(length)
        if self.path == "/api/v1/items":
            self._send(201, "application/json", json.dumps({"created": True}).encode())
        elif self.path == "/v1/chat/completions":
            self._send(401, "application/json",
                       json.dumps({"error": {"message": "missing key", "type": "invalid_request_error"}}).encode())
        else:
            self._send(404, "application/json", b'{"error":"not found"}')


if __name__ == "__main__":
    print("Serving AISec SPA target on http://localhost:3100")
    HTTPServer(("127.0.0.1", 3100), Handler).serve_forever()
