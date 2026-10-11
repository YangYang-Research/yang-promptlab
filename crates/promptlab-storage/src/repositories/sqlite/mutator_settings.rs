use std::collections::BTreeMap;

use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{MutatorSettings, MutatorSettingsRow, UpdateMutatorSettings};
use crate::repositories::MutatorSettingsRepository;
use crate::util::now;

const DEFAULT_ENABLED_JSON: &str = concat!(
    r#"["base64_wrap","unicode_homoglyph","delimiter_injection","role_swap","chunk_split","#,
    r#""json_escape","repeat_amplify","hex_wrap","html_wrap","rot13_wrap","leetspeak","#,
    r#""reversed_text","token_split","markdown_code_fence","zero_width_dense","language_pivot","#,
    r#""refusal_suppression","inject_prefix","url_wrap","caesar_wrap","morse_wrap","fullwidth_ascii","#,
    r#""bidi_override","tag_char_smuggle","zero_width_variants","math_alphanumeric","disemvowel","#,
    r#""expand_before","expand_after","capitalization_shuffle","rephrase","shorten","crossover","#,
    r#""llm_rephrase","llm_crossover","llm_few_shot","llm_transfer"]"#
);

const DEFAULT_CATEGORY_MUTATORS_JSON: &str = concat!(
    r#"{"prompt_injection":["delimiter_injection","language_pivot","role_swap","markdown_code_fence","base64_wrap","html_wrap","hex_wrap","token_split","refusal_suppression","inject_prefix","url_wrap","expand_before","tag_char_smuggle","fullwidth_ascii","bidi_override","rephrase","crossover","llm_rephrase","llm_crossover","llm_few_shot"],"#,
    r#""jailbreak":["role_swap","language_pivot","unicode_homoglyph","base64_wrap","html_wrap","leetspeak","chunk_split","zero_width_dense","rot13_wrap","reversed_text","refusal_suppression","inject_prefix","expand_before","expand_after","morse_wrap","caesar_wrap","math_alphanumeric","disemvowel","capitalization_shuffle","rephrase","crossover","bidi_override","tag_char_smuggle","fullwidth_ascii","zero_width_variants","llm_rephrase","llm_crossover","llm_few_shot","llm_transfer"],"#,
    r#""system_prompt_extraction":["repeat_amplify","language_pivot","delimiter_injection","role_swap","markdown_code_fence","base64_wrap","hex_wrap","refusal_suppression","expand_before","expand_after","url_wrap","disemvowel","rephrase","llm_rephrase","llm_transfer"],"#,
    r#""tool_abuse":["json_escape","html_wrap","base64_wrap","delimiter_injection","token_split","markdown_code_fence","url_wrap","tag_char_smuggle","bidi_override"],"#,
    r#""mcp_abuse":["json_escape","html_wrap","base64_wrap","delimiter_injection","token_split","markdown_code_fence","url_wrap","tag_char_smuggle","bidi_override"],"#,
    r#""rag_leakage":["repeat_amplify","delimiter_injection","markdown_code_fence","zero_width_dense","token_split","zero_width_variants","tag_char_smuggle","refusal_suppression"],"#,
    r#""memory_poisoning":["repeat_amplify","language_pivot","role_swap","delimiter_injection","markdown_code_fence","chunk_split","expand_before","rephrase","crossover"],"#,
    r#""cross_user_leakage":["role_swap","delimiter_injection","repeat_amplify","markdown_code_fence","zero_width_dense","inject_prefix","bidi_override","tag_char_smuggle"],"#,
    r#""agent_goal_hijacking":["role_swap","language_pivot","delimiter_injection","markdown_code_fence","repeat_amplify","token_split","refusal_suppression","expand_after","rephrase","crossover"]}"#
);

fn known_mutator_ids() -> &'static [&'static str] {
    &[
        "base64_wrap",
        "unicode_homoglyph",
        "delimiter_injection",
        "role_swap",
        "chunk_split",
        "json_escape",
        "repeat_amplify",
        "hex_wrap",
        "html_wrap",
        "rot13_wrap",
        "leetspeak",
        "reversed_text",
        "token_split",
        "markdown_code_fence",
        "zero_width_dense",
        "language_pivot",
        "refusal_suppression",
        "inject_prefix",
        "url_wrap",
        "caesar_wrap",
        "morse_wrap",
        "fullwidth_ascii",
        "bidi_override",
        "tag_char_smuggle",
        "zero_width_variants",
        "math_alphanumeric",
        "disemvowel",
        "expand_before",
        "expand_after",
        "capitalization_shuffle",
        "rephrase",
        "shorten",
        "crossover",
        "llm_rephrase",
        "llm_crossover",
        "llm_few_shot",
        "llm_transfer",
    ]
}

fn known_category_ids() -> &'static [&'static str] {
    &[
        "prompt_injection",
        "system_prompt_extraction",
        "jailbreak",
        "rag_leakage",
        "memory_poisoning",
        "cross_user_leakage",
        "agent_goal_hijacking",
        "tool_abuse",
        "mcp_abuse",
    ]
}

fn to_settings(row: MutatorSettingsRow) -> MutatorSettings {
    MutatorSettings {
        id: row.id,
        enabled_mutators: parse_enabled(&row.enabled_mutators_json),
        category_mutators: parse_category_map(&row.category_mutators_json),
        updated_at: row.updated_at,
    }
}

#[derive(Clone)]
pub struct SqliteMutatorSettingsRepository {
    pool: SqlitePool,
}

impl SqliteMutatorSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn sanitize_enabled(raw: impl IntoIterator<Item = String>) -> Vec<String> {
    let known: std::collections::HashSet<&str> = known_mutator_ids().iter().copied().collect();
    let mut cleaned = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for id in raw {
        let normalized = id.trim().to_ascii_lowercase().replace('-', "_");
        if known.contains(normalized.as_str()) && seen.insert(normalized.clone()) {
            cleaned.push(normalized);
        }
    }
    cleaned
}

fn parse_enabled(raw: &str) -> Vec<String> {
    let parsed: Vec<String> = serde_json::from_str(raw).unwrap_or_default();
    sanitize_enabled(parsed)
}

fn sanitize_category_map(
    raw: BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let known_cats: std::collections::HashSet<&str> =
        known_category_ids().iter().copied().collect();
    let mut out = BTreeMap::new();
    for (category, mutators) in raw {
        let cat = category.trim().to_ascii_lowercase().replace('-', "_");
        if !known_cats.contains(cat.as_str()) {
            continue;
        }
        let cleaned = sanitize_enabled(mutators);
        out.insert(cat, cleaned);
    }
    out
}

fn parse_category_map(raw: &str) -> BTreeMap<String, Vec<String>> {
    let parsed: BTreeMap<String, Vec<String>> = serde_json::from_str(raw).unwrap_or_default();
    let cleaned = sanitize_category_map(parsed);
    if cleaned.is_empty() {
        sanitize_category_map(
            serde_json::from_str(DEFAULT_CATEGORY_MUTATORS_JSON).unwrap_or_default(),
        )
    } else {
        cleaned
    }
}

#[async_trait]
impl MutatorSettingsRepository for SqliteMutatorSettingsRepository {
    async fn get(&self) -> PromptLabResult<MutatorSettings> {
        let row = sqlx::query_as::<_, MutatorSettingsRow>(
            r#"
            SELECT id, enabled_mutators_json, category_mutators_json, updated_at
            FROM mutator_settings
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_storage()?;

        if let Some(row) = row {
            return Ok(to_settings(row));
        }

        let timestamp = now();
        sqlx::query(
            r#"
            INSERT INTO mutator_settings (id, enabled_mutators_json, category_mutators_json, updated_at)
            VALUES (1, ?, ?, ?)
            "#,
        )
        .bind(DEFAULT_ENABLED_JSON)
        .bind(DEFAULT_CATEGORY_MUTATORS_JSON)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        Ok(MutatorSettings {
            id: 1,
            enabled_mutators: parse_enabled(DEFAULT_ENABLED_JSON),
            category_mutators: parse_category_map(DEFAULT_CATEGORY_MUTATORS_JSON),
            updated_at: timestamp,
        })
    }

    async fn update(&self, input: UpdateMutatorSettings) -> PromptLabResult<MutatorSettings> {
        let cleaned_enabled = sanitize_enabled(input.enabled_mutators);
        let cleaned_map = sanitize_category_map(input.category_mutators);
        let enabled_json = serde_json::to_string(&cleaned_enabled).unwrap_or_else(|_| "[]".into());
        let map_json = serde_json::to_string(&cleaned_map).unwrap_or_else(|_| "{}".into());
        let timestamp = now();

        let _ = self.get().await?;
        sqlx::query(
            r#"
            UPDATE mutator_settings
            SET enabled_mutators_json = ?, category_mutators_json = ?, updated_at = ?
            WHERE id = 1
            "#,
        )
        .bind(enabled_json)
        .bind(map_json)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;
    use crate::repositories::MutatorSettingsRepository;

    #[tokio::test]
    async fn get_seeds_defaults_and_update_persists_category_map() {
        let db = test_database().await;
        let repo = SqliteMutatorSettingsRepository::new(db.pool().clone());

        let initial = repo.get().await.expect("get");
        assert!(!initial.enabled_mutators.is_empty());
        assert!(initial.category_mutators.contains_key("prompt_injection"));
        assert!(initial
            .category_mutators
            .get("jailbreak")
            .unwrap()
            .contains(&"leetspeak".to_string()));

        let mut map = initial.category_mutators.clone();
        map.insert(
            "prompt_injection".into(),
            vec!["delimiter_injection".into(), "base64_wrap".into()],
        );
        map.insert("jailbreak".into(), vec![]);

        let updated = repo
            .update(UpdateMutatorSettings {
                enabled_mutators: vec!["delimiter_injection".into(), "base64_wrap".into()],
                category_mutators: map,
            })
            .await
            .expect("update");

        assert_eq!(
            updated.enabled_mutators,
            vec!["delimiter_injection".to_string(), "base64_wrap".to_string()]
        );
        assert_eq!(
            updated.category_mutators.get("prompt_injection").unwrap(),
            &vec!["delimiter_injection".to_string(), "base64_wrap".to_string()]
        );
        assert!(updated
            .category_mutators
            .get("jailbreak")
            .unwrap()
            .is_empty());
    }
}
