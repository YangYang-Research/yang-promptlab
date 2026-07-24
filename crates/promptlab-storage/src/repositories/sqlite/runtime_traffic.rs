use async_trait::async_trait;
use sqlx::SqlitePool;

use promptlab_core::PromptLabResult;

use crate::error::StorageResultExt;
use crate::models::{
    CreateRuntimeTrafficEvent, RuntimeTrafficCounters, RuntimeTrafficEvent,
};
use crate::repositories::RuntimeTrafficRepository;
use crate::util::{new_id, now};

#[derive(Clone)]
pub struct SqliteRuntimeTrafficRepository {
    pool: SqlitePool,
}

impl SqliteRuntimeTrafficRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RuntimeTrafficRepository for SqliteRuntimeTrafficRepository {
    async fn insert_many(&self, events: Vec<CreateRuntimeTrafficEvent>) -> PromptLabResult<u64> {
        if events.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await.map_storage()?;
        let timestamp = now();
        let mut inserted = 0u64;
        let mut sent_delta = 0i64;
        let mut received_delta = 0i64;

        for event in events {
            let id = new_id();
            let result = sqlx::query(
                r#"
                INSERT INTO runtime_traffic_events (id, at_ms, direction, created_at)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(&id)
            .bind(event.at_ms)
            .bind(&event.direction)
            .bind(timestamp)
            .execute(&mut *tx)
            .await
            .map_storage()?;
            inserted += result.rows_affected();
            match event.direction.as_str() {
                "sent" => sent_delta += 1,
                "received" => received_delta += 1,
                _ => {}
            }
        }

        sqlx::query(
            r#"
            UPDATE runtime_traffic_counters
            SET lifetime_sent = lifetime_sent + ?,
                lifetime_received = lifetime_received + ?
            WHERE id = 1
            "#,
        )
        .bind(sent_delta)
        .bind(received_delta)
        .execute(&mut *tx)
        .await
        .map_storage()?;

        tx.commit().await.map_storage()?;
        Ok(inserted)
    }

    async fn list_between(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> PromptLabResult<Vec<RuntimeTrafficEvent>> {
        sqlx::query_as::<_, RuntimeTrafficEvent>(
            r#"
            SELECT id, at_ms, direction, created_at
            FROM runtime_traffic_events
            WHERE at_ms >= ? AND at_ms <= ?
            ORDER BY at_ms ASC
            "#,
        )
        .bind(start_ms)
        .bind(end_ms)
        .fetch_all(&self.pool)
        .await
        .map_storage()
    }

    async fn counters(&self) -> PromptLabResult<RuntimeTrafficCounters> {
        sqlx::query_as::<_, RuntimeTrafficCounters>(
            "SELECT id, lifetime_sent, lifetime_received FROM runtime_traffic_counters WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await
        .map_storage()
    }

    async fn prune_before(&self, cutoff_ms: i64) -> PromptLabResult<u64> {
        let result = sqlx::query("DELETE FROM runtime_traffic_events WHERE at_ms < ?")
            .bind(cutoff_ms)
            .execute(&self.pool)
            .await
            .map_storage()?;
        Ok(result.rows_affected())
    }
}
