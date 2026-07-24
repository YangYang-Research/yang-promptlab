"""Base plugin runtime."""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import Any, Callable

from promptlab_plugin.protocol import host_call, read_message, respond, respond_error


@dataclass
class PluginContext:
    plugin_id: str
    plugin_dir: str
    params: dict[str, Any] = field(default_factory=dict)

    def log(self, message: str, level: str = "info") -> None:
        host_call("log", {"level": level, "message": message})

    def emit_finding(self, finding: dict[str, Any]) -> None:
        host_call("emit_finding", finding)


Handler = Callable[[PluginContext], Any]


class Plugin:
    """Base PromptLab plugin with JSON-lines runtime loop."""

    handlers: dict[str, Handler] = {}

    @classmethod
    def register(cls, method: str, handler: Handler) -> None:
        cls.handlers[method] = handler

    @classmethod
    def run(cls) -> None:
        ctx_base = PluginContext(
            plugin_id=os.environ.get("PROMPTLAB_PLUGIN_ID", "unknown"),
            plugin_dir=os.environ.get("PROMPTLAB_PLUGIN_DIR", "."),
        )

        while True:
            message = read_message()
            if message is None:
                break

            if message.get("type") == "shutdown":
                break

            request_id = message.get("id")
            method = message.get("method")
            params = message.get("params") or {}

            if not request_id or not method:
                continue

            handler = cls.handlers.get(method)
            if handler is None:
                respond_error(request_id, f"unknown method: {method}")
                continue

            ctx = PluginContext(
                plugin_id=ctx_base.plugin_id,
                plugin_dir=ctx_base.plugin_dir,
                params=params,
            )

            try:
                result = handler(ctx)
                respond(request_id, result)
            except Exception as exc:  # noqa: BLE001 — plugin boundary
                respond_error(request_id, str(exc))
            break
