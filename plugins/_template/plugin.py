#!/usr/bin/env python3
"""AISec plugin template — install SDK: pip install -e packages/plugin-sdk-python"""

from aisec_plugin.discovery import DiscoveryPlugin


@DiscoveryPlugin.register("discover")
def discover(ctx):
    ctx.log("starting discovery")
    return {"endpoints": [], "count": 0}


if __name__ == "__main__":
    DiscoveryPlugin.run()
