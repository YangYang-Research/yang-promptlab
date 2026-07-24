"""AISec Plugin SDK for Python."""

from aisec_plugin.attack import AttackPlugin
from aisec_plugin.base import Plugin, PluginContext
from aisec_plugin.discovery import DiscoveryPlugin
from aisec_plugin.judge import JudgePlugin
from aisec_plugin.report import ReportPlugin

__all__ = [
    "Plugin",
    "PluginContext",
    "DiscoveryPlugin",
    "AttackPlugin",
    "JudgePlugin",
    "ReportPlugin",
]

__version__ = "0.1.0"
