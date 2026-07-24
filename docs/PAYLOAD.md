# PromptLab Payload Engine

**Crate:** `promptlab-payload`  
**Purpose:** Static payload library, encoding mutations, and variant generation pipeline.

---

## Components

| Component | Description |
|-----------|-------------|
| **PayloadDatabase** | Embedded JSON catalog (`data/payloads.json`), 24+ static payloads |
| **MutationEngine** | Unicode, Base64, Hex, HTML encoding + wrap variants |
| **PayloadPipeline** | Library selection → mutation → `GeneratedPayload` output |

---

## Payload Database

Catalog is embedded at compile time via `include_str!`. Each record:

```json
{
  "id": "pi-direct-override",
  "name": "Direct instruction override",
  "category": "prompt_injection",
  "tags": ["direct", "override"],
  "content": "..."
}
```

Query API:

```rust
let db = PayloadDatabase::builtin()?;
db.get("pi-direct-override");
db.by_category(PayloadCategory::Jailbreak);
db.by_tag("mcp");
```

---

## Mutation Engine

| Mutation | Effect |
|----------|--------|
| `UnicodeObfuscation` | Cyrillic homoglyphs + zero-width insertion |
| `Base64Encode` | Standard base64 of UTF-8 bytes |
| `HexEncode` | Lowercase hex of UTF-8 bytes |
| `HtmlEncode` | HTML entity encoding (`&lt;`, `&#NN;`) |
| `Base64Wrap` | Base64 + decode instruction wrapper |
| `HexWrap` | Hex + decode instruction wrapper |
| `HtmlWrap` | HTML entities + decode instruction wrapper |

```rust
use promptlab_payload::{MutationEngine, MutationKind};

let engine = MutationEngine::with_defaults();
let encoded = engine.apply(MutationKind::HexEncode, "secret")?;
let chain = engine.apply_chain(
    &[MutationKind::UnicodeObfuscation, MutationKind::Base64Encode],
    "secret",
)?;
let variants = engine.expand("secret", MutationKind::encoding_kinds())?;
```

---

## Generation Pipeline

```rust
use promptlab_payload::{
    GenerateRequest, MutationKind, PayloadCategory, PayloadPipeline,
};

let pipeline = PayloadPipeline::with_defaults()?;

let report = pipeline.generate(&GenerateRequest {
    categories: Some(vec![PayloadCategory::PromptInjection]),
    mutations: vec![
        MutationKind::UnicodeObfuscation,
        MutationKind::Base64Encode,
        MutationKind::HtmlEncode,
    ],
    max_variants_per_payload: Some(5),
    ..Default::default()
})?;

for variant in &report.variants {
    println!("{}: {}", variant.source_id, variant.content);
}
```

Filter options: `categories`, `payload_ids`, `tags`, `mutations`.

---

## Tests

```bash
cargo test -p promptlab-payload
```

---

## Integration

- **`promptlab-attack`** — consume `GeneratedPayload` variants in attack executor
- **`promptlab-storage`** — optional `storage` feature for persisting custom payloads to SQLite `payloads` table
