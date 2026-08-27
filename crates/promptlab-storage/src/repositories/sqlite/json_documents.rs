//! Singleton JSON document tables (token_usage, recent_activity, yazg_chat_threads).

use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;
use time::OffsetDateTime;

use crate::error::StorageResultExt;
use crate::util::now;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonDocumentRecord {
    pub id: String,
    pub data_json: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct JsonDocumentRow {
    id: String,
    data_json: String,
    updated_at: OffsetDateTime,
}

fn to_record(row: JsonDocumentRow) -> JsonDocumentRecord {
    JsonDocumentRecord {
        id: row.id,
        data_json: row.data_json,
        updated_at: row.updated_at,
    }
}

#[async_trait]
pub trait JsonDocumentRepository: Send + Sync {
    async fn get(&self) -> PromptLabResult<Option<JsonDocumentRecord>>;
    async fn upsert(&self, data_json: &str) -> PromptLabResult<JsonDocumentRecord>;
}

macro_rules! json_document_repo {
    ($name:ident, $table:literal, $id:expr) => {
        #[derive(Clone)]
        pub struct $name {
            pool: SqlitePool,
        }

        impl $name {
            pub fn new(pool: SqlitePool) -> Self {
                Self { pool }
            }
        }

        #[async_trait]
        impl JsonDocumentRepository for $name {
            async fn get(&self) -> PromptLabResult<Option<JsonDocumentRecord>> {
                let row = sqlx::query_as::<_, JsonDocumentRow>(concat!(
                    "SELECT id, data_json, updated_at FROM ",
                    $table,
                    " WHERE id = ?"
                ))
                .bind($id)
                .fetch_optional(&self.pool)
                .await
                .map_storage()?;
                Ok(row.map(to_record))
            }

            async fn upsert(&self, data_json: &str) -> PromptLabResult<JsonDocumentRecord> {
                let timestamp = now();
                sqlx::query(concat!(
                    "INSERT INTO ",
                    $table,
                    " (id, data_json, updated_at) VALUES (?, ?, ?) ",
                    "ON CONFLICT(id) DO UPDATE SET ",
                    "data_json = excluded.data_json, updated_at = excluded.updated_at"
                ))
                .bind($id)
                .bind(data_json)
                .bind(timestamp)
                .execute(&self.pool)
                .await
                .map_storage()?;

                Ok(JsonDocumentRecord {
                    id: ($id).to_string(),
                    data_json: data_json.to_string(),
                    updated_at: timestamp,
                })
            }
        }
    };
}

pub const TOKEN_USAGE_ID: &str = "default";
pub const RECENT_ACTIVITY_ID: &str = "default";
pub const YAZG_CHAT_THREADS_ID: &str = "default";

json_document_repo!(SqliteTokenUsageRepository, "token_usage", TOKEN_USAGE_ID);
json_document_repo!(
    SqliteRecentActivityRepository,
    "recent_activity",
    RECENT_ACTIVITY_ID
);
json_document_repo!(
    SqliteYazgChatThreadsRepository,
    "yazg_chat_threads",
    YAZG_CHAT_THREADS_ID
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::test_database;

    #[tokio::test]
    async fn token_usage_roundtrip() {
        let db = test_database().await;
        let repo = SqliteTokenUsageRepository::new(db.pool().clone());
        assert!(repo.get().await.expect("get").is_none());
        repo.upsert(r#"{"version":1}"#).await.expect("upsert");
        assert_eq!(
            repo.get().await.expect("get").expect("row").data_json,
            r#"{"version":1}"#
        );
    }

    #[tokio::test]
    async fn recent_activity_and_yazg_tables() {
        let db = test_database().await;
        let activity = SqliteRecentActivityRepository::new(db.pool().clone());
        let yazg = SqliteYazgChatThreadsRepository::new(db.pool().clone());
        activity.upsert("[]").await.expect("activity");
        yazg.upsert(r#"{"threads":[]}"#).await.expect("yazg");
        assert_eq!(
            activity.get().await.expect("get").expect("row").data_json,
            "[]"
        );
        assert!(yazg
            .get()
            .await
            .expect("get")
            .expect("row")
            .data_json
            .contains("threads"));
    }
}
