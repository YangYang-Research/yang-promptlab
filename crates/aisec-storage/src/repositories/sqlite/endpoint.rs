use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::error::StorageResultExt;
use crate::models::{CreateEndpoint, Endpoint, UpdateEndpoint};
use crate::repositories::EndpointRepository;
use crate::util::{new_id, now};

#[derive(Clone)]
pub struct SqliteEndpointRepository {
    pool: SqlitePool,
}

impl SqliteEndpointRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EndpointRepository for SqliteEndpointRepository {
    async fn create(&self, input: CreateEndpoint) -> AisecResult<Endpoint> {
        let id = new_id();
        let created_at = now();
        let endpoint_type = input
            .endpoint_type
            .unwrap_or_else(|| "unknown_ai".into());
        let risk_score = input.risk_score.unwrap_or(0);
        let metadata_confidence = input.metadata_confidence.unwrap_or(0.0);
        let discovery_source = input
            .discovery_source
            .unwrap_or_else(|| "discovery".into());
        let auth_required = i64::from(input.auth_required.unwrap_or(false));

        sqlx::query(
            r#"
            INSERT INTO endpoints (
                id, scan_id, target_id, url, kind, method,
                confidence, evidence, source_url, discovered_at, created_at,
                metadata_json, endpoint_type, ai_framework, risk_score,
                metadata_confidence, discovery_source, auth_required
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.scan_id)
        .bind(&input.target_id)
        .bind(&input.url)
        .bind(&input.kind)
        .bind(&input.method)
        .bind(input.confidence)
        .bind(&input.evidence)
        .bind(&input.source_url)
        .bind(input.discovered_at)
        .bind(created_at)
        .bind(&input.metadata_json)
        .bind(&endpoint_type)
        .bind(&input.ai_framework)
        .bind(risk_score)
        .bind(metadata_confidence)
        .bind(&discovery_source)
        .bind(auth_required)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get(&id).await
    }

    async fn create_many(&self, inputs: Vec<CreateEndpoint>) -> AisecResult<Vec<Endpoint>> {
        let mut created = Vec::with_capacity(inputs.len());
        for input in inputs {
            created.push(self.create(input).await?);
        }
        Ok(created)
    }

    async fn get(&self, id: &str) -> AisecResult<Endpoint> {
        sqlx::query_as::<_, Endpoint>("SELECT * FROM endpoints WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
    }

    async fn list_by_scan(&self, scan_id: &str) -> AisecResult<Vec<Endpoint>> {
        sqlx::query_as::<_, Endpoint>(
            "SELECT * FROM endpoints WHERE scan_id = ? ORDER BY risk_score DESC, confidence DESC, url ASC",
        )
        .bind(scan_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn update(&self, id: &str, input: UpdateEndpoint) -> AisecResult<Endpoint> {
        if let Some(method) = &input.method {
            sqlx::query("UPDATE endpoints SET method = ? WHERE id = ?")
                .bind(method)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_storage()?;
        }
        self.get(id).await
    }

    async fn delete_by_scan(&self, scan_id: &str) -> AisecResult<u64> {
        let result = sqlx::query("DELETE FROM endpoints WHERE scan_id = ?")
            .bind(scan_id)
            .execute(&self.pool)
            .await
            .map_storage()?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CreateProject, CreateScan};
    use crate::pool::test_utils::test_database;
    use crate::repositories::{ProjectRepository, ScanRepository};

    #[tokio::test]
    async fn endpoint_persist_and_list() {
        let db = test_database().await;
        let repos = db.repositories();

        let project = repos
            .projects()
            .create(CreateProject {
                name: "proj".into(),
                description: None,
            })
            .await
            .unwrap();

        let scan = repos
            .scans()
            .create(CreateScan {
                project_id: project.id,
                target_id: None,
                name: "discovery".into(),
                status: Some("running".into()),
                playbook_json: None,
            })
            .await
            .unwrap();

        let created = repos
            .endpoints()
            .create_many(vec![
                CreateEndpoint {
                    scan_id: scan.id.clone(),
                    target_id: None,
                    url: "https://example.com/v1/chat/completions".into(),
                    kind: "ai_endpoint".into(),
                    method: Some("POST".into()),
                    confidence: 0.9,
                    evidence: Some("known AI path".into()),
                    source_url: Some("https://example.com/".into()),
                    discovered_at: now(),
                    metadata_json: Some(r#"{"basic":{"url":"https://example.com/v1/chat/completions"}}"#.into()),
                    endpoint_type: Some("ai_chat".into()),
                    ai_framework: Some("openai".into()),
                    risk_score: Some(72),
                    metadata_confidence: Some(0.9),
                    discovery_source: Some("discovery".into()),
                    auth_required: Some(false),
                },
            ])
            .await
            .unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].endpoint_type, "ai_chat");
        assert_eq!(created[0].risk_score, 72);

        let listed = repos.endpoints().list_by_scan(&scan.id).await.unwrap();
        assert_eq!(listed.len(), 1);
    }
}
