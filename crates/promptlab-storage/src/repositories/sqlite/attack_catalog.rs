use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::error::StorageResultExt;
use crate::models::{
    AttackCatalogTechnique, UpdateAttackCatalogTechnique, UpsertAttackCatalogTechnique,
};
use crate::repositories::AttackCatalogRepository;
use crate::util::{ensure_rows_affected, now};

#[derive(Clone)]
pub struct SqliteAttackCatalogRepository {
    pool: SqlitePool,
}

impl SqliteAttackCatalogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AttackCatalogRepository for SqliteAttackCatalogRepository {
    async fn get(&self, id: &str) -> AisecResult<AttackCatalogTechnique> {
        sqlx::query_as::<_, AttackCatalogTechnique>(
            "SELECT * FROM attack_catalog_techniques WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_storage()
    }

    async fn list(&self) -> AisecResult<Vec<AttackCatalogTechnique>> {
        sqlx::query_as::<_, AttackCatalogTechnique>(
            r#"
            SELECT * FROM attack_catalog_techniques
            ORDER BY category_id ASC, sort_order ASC, name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn list_enabled(&self) -> AisecResult<Vec<AttackCatalogTechnique>> {
        sqlx::query_as::<_, AttackCatalogTechnique>(
            r#"
            SELECT * FROM attack_catalog_techniques
            WHERE enabled = 1
            ORDER BY category_id ASC, sort_order ASC, name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn list_by_category(&self, category_id: &str) -> AisecResult<Vec<AttackCatalogTechnique>> {
        sqlx::query_as::<_, AttackCatalogTechnique>(
            r#"
            SELECT * FROM attack_catalog_techniques
            WHERE category_id = ?
            ORDER BY sort_order ASC, name ASC
            "#,
        )
        .bind(category_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn seed_from(&self, entries: Vec<UpsertAttackCatalogTechnique>) -> AisecResult<u64> {
        let mut touched = 0u64;
        let timestamp = now();

        for entry in entries {
            let existing = sqlx::query_as::<_, AttackCatalogTechnique>(
                "SELECT * FROM attack_catalog_techniques WHERE id = ?",
            )
            .bind(&entry.id)
            .fetch_optional(&self.pool)
            .await
            .map_storage()?;

            match existing {
                None => {
                    sqlx::query(
                        r#"
                        INSERT INTO attack_catalog_techniques (
                            id, category_id, name, description, content, default_content,
                            tags_json, surface, owasp, enabled, user_modified, sort_order,
                            created_at, updated_at
                        )
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
                        "#,
                    )
                    .bind(&entry.id)
                    .bind(&entry.category_id)
                    .bind(&entry.name)
                    .bind(&entry.description)
                    .bind(&entry.content)
                    .bind(&entry.default_content)
                    .bind(&entry.tags_json)
                    .bind(&entry.surface)
                    .bind(&entry.owasp)
                    .bind(entry.enabled)
                    .bind(entry.sort_order)
                    .bind(timestamp)
                    .bind(timestamp)
                    .execute(&self.pool)
                    .await
                    .map_storage()?;
                    touched += 1;
                }
                Some(row) if !row.user_modified => {
                    sqlx::query(
                        r#"
                        UPDATE attack_catalog_techniques
                        SET category_id = ?, name = ?, description = ?, content = ?,
                            default_content = ?, tags_json = ?, surface = ?, owasp = ?,
                            enabled = ?, sort_order = ?, updated_at = ?
                        WHERE id = ?
                        "#,
                    )
                    .bind(&entry.category_id)
                    .bind(&entry.name)
                    .bind(&entry.description)
                    .bind(&entry.content)
                    .bind(&entry.default_content)
                    .bind(&entry.tags_json)
                    .bind(&entry.surface)
                    .bind(&entry.owasp)
                    .bind(entry.enabled)
                    .bind(entry.sort_order)
                    .bind(timestamp)
                    .bind(&entry.id)
                    .execute(&self.pool)
                    .await
                    .map_storage()?;
                    touched += 1;
                }
                Some(_) => {
                    // Keep user content; refresh factory default + metadata for Reset.
                    sqlx::query(
                        r#"
                        UPDATE attack_catalog_techniques
                        SET category_id = ?, name = ?, description = ?,
                            default_content = ?, tags_json = ?, surface = ?, owasp = ?,
                            sort_order = ?, updated_at = ?
                        WHERE id = ?
                        "#,
                    )
                    .bind(&entry.category_id)
                    .bind(&entry.name)
                    .bind(&entry.description)
                    .bind(&entry.default_content)
                    .bind(&entry.tags_json)
                    .bind(&entry.surface)
                    .bind(&entry.owasp)
                    .bind(entry.sort_order)
                    .bind(timestamp)
                    .bind(&entry.id)
                    .execute(&self.pool)
                    .await
                    .map_storage()?;
                    touched += 1;
                }
            }
        }

        Ok(touched)
    }

    async fn update(
        &self,
        id: &str,
        input: UpdateAttackCatalogTechnique,
    ) -> AisecResult<AttackCatalogTechnique> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let content = input.content.unwrap_or_else(|| existing.content.clone());
        let enabled = input.enabled.unwrap_or(existing.enabled);
        let tags_json = input.tags_json.unwrap_or(existing.tags_json);
        let surface = input.surface.or(existing.surface);
        let owasp = input.owasp.or(existing.owasp);
        let sort_order = input.sort_order.unwrap_or(existing.sort_order);
        let user_modified = content != existing.default_content;
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE attack_catalog_techniques
            SET name = ?, description = ?, content = ?, enabled = ?, tags_json = ?,
                surface = ?, owasp = ?, user_modified = ?, sort_order = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&description)
        .bind(&content)
        .bind(enabled)
        .bind(&tags_json)
        .bind(&surface)
        .bind(&owasp)
        .bind(user_modified)
        .bind(sort_order)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "attack catalog technique")?;
        self.get(id).await
    }

    async fn reset_content(&self, id: &str) -> AisecResult<AttackCatalogTechnique> {
        let existing = self.get(id).await?;
        let updated_at = now();
        let result = sqlx::query(
            r#"
            UPDATE attack_catalog_techniques
            SET content = default_content, user_modified = 0, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "attack catalog technique")?;
        let _ = existing;
        self.get(id).await
    }
}
