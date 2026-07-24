#!/usr/bin/env python3
"""PromptLab auth-engine demo target — serves http://localhost:3200

A minimal username/password login flow:
  GET  /login      -> HTML form (#user, #pass, #submit)
  POST /login      -> sets a `session` cookie, 302 redirect to /dashboard
  GET  /dashboard  -> protected page that stores an access_token in localStorage
"""

from http.server import HTTPServer, BaseHTTPRequestHandler

LOGIN_HTML = """<!doctype html>
<html><body>
  <h1>Sign in</h1>
  <form method="post" action="/login">
    <input id="user" name="username" placeholder="username" />
    <input id="pass" name="password" type="password" placeholder="password" />
    <button id="submit" type="submit">Login</button>
  </form>
</body></html>"""

DASHBOARD_HTML = """<!doctype html>
<html><body>
  <h1>Dashboard</h1>
  <p>Welcome, you are authenticated.</p>
  <script>
    // App stores an access token in localStorage after login.
    localStorage.setItem('access_token', 'tok-xyz-12345');
  </script>
</body></html>"""


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[auth-target] {args[0]}")

    def _send(self, code, ctype, body, extra_headers=None):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for k, v in (extra_headers or []):
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/login":
            self._send(200, "text/html", LOGIN_HTML.encode())
        elif self.path == "/dashboard":
            self._send(200, "text/html", DASHBOARD_HTML.encode())
        else:
            self._send(404, "text/plain", b"not found")

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        if length:
            self.rfile.read(length)
        if self.path == "/login":
            # Issue a session cookie and redirect to the protected page.
            self.send_response(302)
            self.send_header("Set-Cookie", "session=sess-abc-789; Path=/; HttpOnly")
            self.send_header("Location", "/dashboard")
            self.send_header("Content-Length", "0")
            self.end_headers()
        else:
            self._send(404, "text/plain", b"not found")


if __name__ == "__main__":
    print("Serving PromptLab auth login target on http://localhost:3200")
    HTTPServer(("127.0.0.1", 3200), Handler).serve_forever()
