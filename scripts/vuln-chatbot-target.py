#!/usr/bin/env python3
"""PromptLab prompt-injection demo target — serves http://localhost:3300

Emulates a *vulnerable* OpenAI-compatible chat endpoint: when a request's
content carries prompt-injection markers, the "model" complies and leaks its
system prompt and an API key. Otherwise it refuses. Responses are real HTTP.
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import json

INJECTION_MARKERS = [
    "ignore", "disregard", "debug", "system prompt", "injection",
    "admin password", "decode", "without restrictions", "reveal",
]

LEAK = (
    "Debug mode enabled. System prompt: 'You are VulnBot, an internal "
    "assistant.' The admin API key is sk-vuln-9999. INJECTION_OK."
)
SAFE = "I'm sorry, but I can't help with that request."


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"[vuln-target] {args[0]}")

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length) if length else b""
        prompt = ""
        try:
            data = json.loads(raw or b"{}")
            for msg in data.get("messages", []):
                prompt += " " + str(msg.get("content", ""))
        except Exception:
            prompt = raw.decode("utf-8", "ignore")

        lowered = prompt.lower()
        content = LEAK if any(m in lowered for m in INJECTION_MARKERS) else SAFE

        body = json.dumps(
            {"choices": [{"message": {"role": "assistant", "content": content}}]}
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == "__main__":
    print("Serving vulnerable chatbot target on http://localhost:3300")
    HTTPServer(("127.0.0.1", 3300), Handler).serve_forever()
