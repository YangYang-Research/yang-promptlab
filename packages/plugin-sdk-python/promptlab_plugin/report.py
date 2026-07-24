"""Report plugin base."""

from __future__ import annotations

from typing import Any

from aisec_plugin.base import Plugin, PluginContext


class ReportPlugin(Plugin):
    @classmethod
    def render_report(cls, ctx: PluginContext) -> dict[str, Any]:
        raise NotImplementedError

    @classmethod
    def setup_handlers(cls) -> None:
        cls.register("render_report", cls.render_report)
