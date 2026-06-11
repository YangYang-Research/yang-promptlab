#!/usr/bin/env python3
"""Sample discovery plugin — suggests OpenAPI / AI paths to probe."""

from __future__ import annotations

import json
import os
import sys

COMMON_PATHS = [
    "/openapi.json",
    "/swagger.json",
    "/v1/openapi.json",
    "/v1/models",
    "/v1/chat/completions",
    "/api/tags",
    "/health",
    "/.well-known/openapi",
]


def read_message():
    line = sys.stdin.readline()
    if not line:
        return None
    line = line.strip()
    if not line:
        return read_message()
    return json.loads(line)


def write_message(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


def host_call(method, params):
    write_message({"type": "host", "method": method, "params": params})


def discover(params):
    seed = params.get("target_url") or params.get("url") or "https://example.com"
    base = seed.rstrip("/")

    endpoints = [
        {"url": f"{base}{path}", "kind": "openapi" if "openapi" in path else "ai_api"}
        for path in COMMON_PATHS
    ]

    host_call("log", {"level": "info", "message": f"discovered {len(endpoints)} candidate paths"})

    for ep in endpoints[:3]:
        host_call(
            "emit_finding",
            {
                "title": f"Candidate endpoint: {ep['url']}",
                "severity": "info",
                "category": "discovery",
                "description": f"Suggested probe target ({ep['kind']})",
            },
        )

    return {"seed_url": seed, "endpoints": endpoints, "count": len(endpoints)}


def main():
    os.environ.get("AISEC_PLUGIN_ID", "com.aisec.sample.discovery-openapi")
    while True:
        msg = read_message()
        if msg is None or msg.get("type") == "shutdown":
            break
        req_id = msg.get("id")
        method = msg.get("method")
        params = msg.get("params") or {}
        if method == "discover" and req_id:
            try:
                result = discover(params)
                write_message({"id": req_id, "result": result})
            except Exception as exc:
                write_message({"id": req_id, "error": {"message": str(exc)}})
            break


if __name__ == "__main__":
    main()
