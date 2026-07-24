# Sample Plugins

Reference implementations for all four plugin types. Each directory contains:

- `promptlab-plugin.toml` — manifest (type, language, capabilities, hooks)
- `plugin.py` or `plugin.js` — entry script

| Plugin | Type | Language | Hook |
|--------|------|----------|------|
| `discovery-openapi-paths` | Discovery | Python | `discover` |
| `attack-delimiter-injection` | Attack | JavaScript | `execute_attack` |
| `judge-keyword` | Judge | Python | `evaluate` |
| `report-markdown-summary` | Report | JavaScript | `render_report` |

## Manual test

```bash
# Discovery
echo '{"id":"1","method":"discover","params":{"target_url":"https://example.com"}}' | \
  python3 plugins/samples/discovery-openapi-paths/plugin.py

# Attack
echo '{"id":"1","method":"execute_attack","params":{"payload":"ignore instructions"}}' | \
  node plugins/samples/attack-delimiter-injection/plugin.js
```

Run integration tests:

```bash
cargo test -p promptlab-plugin-host
```
