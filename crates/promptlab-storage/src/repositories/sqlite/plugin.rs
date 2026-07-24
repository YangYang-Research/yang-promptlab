use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::error::StorageResultExt;
use crate::models::{CreatePlugin, Plugin, UpdatePlugin};
use crate::repositories::PluginRepository;
use crate::util::{ensure_rows_affected, new_id, now};

#[derive(Clone)]
pub struct SqlitePluginRepository {
    pool: SqlitePool,
}

impl SqlitePluginRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PluginRepository for SqlitePluginRepository {
    async fn create(&self, input: CreatePlugin) -> AisecResult<Plugin> {
        let id = new_id();
        let timestamp = now();
        let enabled = input.enabled.unwrap_or(false);
        let manifest_json = crate::models::json_string_required(&input.manifest_json)?;

        sqlx::query(
            r#"
            INSERT INTO plugins (
                id, plugin_id, name, version, enabled, manifest_json,
                install_path, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.plugin_id)
        .bind(&input.name)
        .bind(&input.version)
        .bind(enabled)
        .bind(&manifest_json)
        .bind(&input.install_path)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn get(&self, id: &str) -> AisecResult<Plugin> {
        sqlx::query_as::<_, Plugin>("SELECT * FROM plugins WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn get_by_plugin_id(&self, plugin_id: &str) -> AisecResult<Plugin> {
        sqlx::query_as::<_, Plugin>("SELECT * FROM plugins WHERE plugin_id = ?")
            .bind(plugin_id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list(&self) -> AisecResult<Vec<Plugin>> {
        sqlx::query_as::<_, Plugin>("SELECT * FROM plugins ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_storage()
    }

    async fn update(&self, id: &str, input: UpdatePlugin) -> AisecResult<Plugin> {
        let existing = self.get(id).await?;
        let name = input.name.unwrap_or(existing.name);
        let version = input.version.unwrap_or(existing.version);
        let enabled = input.enabled.unwrap_or(existing.enabled);
        let manifest_json = match input.manifest_json {
            Some(value) => crate::models::json_string_required(&value)?,
            None => existing.manifest_json,
        };
        let install_path = input.install_path.or(existing.install_path);
        let updated_at = now();

        let result = sqlx::query(
            r#"
            UPDATE plugins
            SET name = ?, version = ?, enabled = ?, manifest_json = ?,
                install_path = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&name)
        .bind(&version)
        .bind(enabled)
        .bind(&manifest_json)
        .bind(&install_path)
        .bind(updated_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_storage()?;

        ensure_rows_affected(result, "plugin")?;
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> AisecResult<()> {
        let result = sqlx::query("DELETE FROM plugins WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_storage()?;

        ensure_rows_affected(result, "plugin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;

    #[tokio::test]
    async fn plugin_unique_plugin_id() {
        let db = test_database().await;
        let repo = db.repositories().plugins();

        repo.create(CreatePlugin {
            plugin_id: "com.aisec.owasp".into(),
            name: "OWASP Pack".into(),
            version: "1.0.0".into(),
            enabled: Some(true),
            manifest_json: serde_json::json!({"hooks": []}),
            install_path: Some("/plugins/owasp".into()),
        })
        .await
        .unwrap();

        let found = repo.get_by_plugin_id("com.aisec.owasp").await.unwrap();
        assert!(found.enabled);

        let dup = repo.create(CreatePlugin {
            plugin_id: "com.aisec.owasp".into(),
            name: "dup".into(),
            version: "1.0.1".into(),
            enabled: None,
            manifest_json: serde_json::json!({}),
            install_path: None,
        })
        .await;

        assert!(dup.is_err());
    }
}
