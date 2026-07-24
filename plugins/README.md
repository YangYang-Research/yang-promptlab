# PromptLab Plugins

Third-party and reference PromptLab plugins live under this directory. The **Plugin Manager** (`promptlab-plugin-host`) discovers manifests recursively and executes plugins in a sandboxed subprocess.

See [docs/PLUGINS.md](../docs/PLUGINS.md) for the full developer guide.

## Layout

```
plugins/
├── README.md
├── _template/              # Starter manifest + entry script
└── samples/                # Reference plugins (Python + JavaScript)
    ├── discovery-openapi-paths/
    ├── attack-delimiter-injection/
    ├── judge-keyword/
    └── report-markdown-summary/
```

## Quick start

1. Copy `_template/` to a new directory (e.g. `plugins/my-vendor/my-plugin/`).
2. Edit `promptlab-plugin.toml` — set `plugin_type`, `language`, capabilities, and hooks.
3. Implement the hook handler using the SDK:
   - Python: `packages/plugin-sdk-python`
   - JavaScript: `packages/plugin-sdk-js`
4. Enable and invoke via `PluginManager` or future IPC `plugin.*` commands.

Sample plugins are self-contained (no SDK install required) and demonstrate the JSON-lines protocol directly.
