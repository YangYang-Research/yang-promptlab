//! Seed and load the global attack technique catalog from SQLite.

use promptlab_core::PromptLabError;
use promptlab_payload::{parse_category, PayloadDatabase, PayloadRecord};
use promptlab_storage::{
    AttackCatalogRepository, AttackCatalogTechnique, Database, Repositories,
    UpsertAttackCatalogTechnique,
};

use crate::error::CommandResult;

pub async fn seed_attack_catalog(database: &Database) -> CommandResult<u64> {
    let entries = PayloadDatabase::seed_entries()
        .map_err(|e| PromptLabError::internal(format!("catalog seed load failed: {e}")))?;
    let upserts: Vec<UpsertAttackCatalogTechnique> = entries
        .into_iter()
        .map(|entry| {
            let tags_json = serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".into());
            UpsertAttackCatalogTechnique {
                id: entry.id,
                category_id: entry.category,
                name: entry.name,
                description: entry.description,
                content: entry.content.clone(),
                default_content: entry.content,
                tags_json,
                surface: entry.surface,
                owasp: entry.owasp,
                enabled: true,
                sort_order: entry.sort_order,
            }
        })
        .collect();

    let touched = database
        .repositories()
        .attack_catalog()
        .seed_from(upserts)
        .await
        .map_err(PromptLabError::from)?;
    tracing::info!(touched, "attack catalog seeded");
    Ok(touched)
}

pub async fn load_payload_database(database: &Database) -> CommandResult<PayloadDatabase> {
    load_payload_database_from_repos(&database.repositories()).await
}

pub async fn load_payload_database_from_repos(
    repos: &Repositories,
) -> CommandResult<PayloadDatabase> {
    let rows = repos
        .attack_catalog()
        .list_enabled()
        .await
        .map_err(PromptLabError::from)?;
    payload_database_from_rows(rows)
}

pub fn payload_database_from_rows(
    rows: Vec<AttackCatalogTechnique>,
) -> CommandResult<PayloadDatabase> {
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let category = parse_category(&row.category_id)
            .map_err(|e| PromptLabError::invalid_input(format!("catalog category: {e}")))?;
        let tags: Vec<String> = serde_json::from_str(&row.tags_json).unwrap_or_default();
        records.push(PayloadRecord {
            id: row.id,
            name: row.name,
            category,
            content: row.content,
            tags,
            description: row.description,
        });
    }
    PayloadDatabase::from_records(2, records)
        .map_err(|e| PromptLabError::internal(format!("catalog build failed: {e}")).into())
}
