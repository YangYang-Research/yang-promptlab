use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::AisecResult;

use crate::error::StorageResultExt;
use crate::models::{CreateEndpoint, Endpoint};
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

        sqlx::query(
            r#"
            INSERT INTO endpoints (
                id, scan_id, target_id, url, kind, method,
                confidence, evidence, source_url, discovered_at, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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

    async fn list_by_scan(&self, scan_id: &str) -> AisecResult<Vec<Endpoint>> {
        sqlx::query_as::<_, Endpoint>(
            "SELECT * FROM endpoints WHERE scan_id = ? ORDER BY confidence DESC, url ASC",
        )
        .bind(scan_id)
        .fetch_all(&self.pool)
        .await
        .map_storage()
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

impl SqliteEndpointRepository {
    async fn get(&self, id: &str) -> AisecResult<Endpoint> {
        sqlx::query_as::<_, Endpoint>("SELECT * FROM endpoints WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_storage()
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
                },
                CreateEndpoint {
                    scan_id: scan.id.clone(),
                    target_id: None,
                    url: "https://example.com/openapi.json".into(),
                    kind: "openapi".into(),
                    method: Some("GET".into()),
                    confidence: 0.95,
                    evidence: Some("openapi marker".into()),
                    source_url: None,
                    discovered_at: now(),
                },
            ])
            .await
            .unwrap();
        assert_eq!(created.len(), 2);

        let listed = repos.endpoints().list_by_scan(&scan.id).await.unwrap();
        assert_eq!(listed.len(), 2);
        // Ordered by confidence DESC.
        assert_eq!(listed[0].kind, "openapi");

        let removed = repos.endpoints().delete_by_scan(&scan.id).await.unwrap();
        assert_eq!(removed, 2);
        assert!(repos.endpoints().list_by_scan(&scan.id).await.unwrap().is_empty());
    }
}
