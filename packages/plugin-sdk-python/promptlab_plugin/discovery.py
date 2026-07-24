"""Discovery plugin base."""

from __future__ import annotations

from typing import Any

from promptlab_plugin.base import Plugin, PluginContext


class DiscoveryPlugin(Plugin):
    @classmethod
    def discover(cls, ctx: PluginContext) -> dict[str, Any]:
        raise NotImplementedError

    @classmethod
    def setup_handlers(cls) -> None:
        cls.register("discover", cls.discover)
