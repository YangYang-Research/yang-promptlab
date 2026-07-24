"""Attack plugin base."""

from __future__ import annotations

from typing import Any

from aisec_plugin.base import Plugin, PluginContext


class AttackPlugin(Plugin):
    @classmethod
    def execute_attack(cls, ctx: PluginContext) -> dict[str, Any]:
        raise NotImplementedError

    @classmethod
    def setup_handlers(cls) -> None:
        cls.register("execute_attack", cls.execute_attack)
