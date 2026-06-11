"""JSON-lines protocol helpers."""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from typing import Any


@dataclass
class HostRequest:
    method: str
    params: dict[str, Any]


def read_message() -> dict[str, Any] | None:
    line = sys.stdin.readline()
    if not line:
        return None
    line = line.strip()
    if not line:
        return read_message()
    return json.loads(line)


def write_message(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def respond(request_id: str, result: Any) -> None:
    write_message({"id": request_id, "result": result})


def respond_error(request_id: str, message: str) -> None:
    write_message({"id": request_id, "error": {"message": message}})


def host_call(method: str, params: dict[str, Any] | None = None) -> None:
    write_message(
        {
            "type": "host",
            "method": method,
            "params": params or {},
        }
    )
