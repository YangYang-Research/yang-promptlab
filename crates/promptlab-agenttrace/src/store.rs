//! SQLite-backed AgentTrace store.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tracing::warn;
use uuid::Uuid;

use crate::error::{AgentTraceError, Result};
use crate::types::{
    ExperimentRecord, ListTracesFilter, SpanEnd, SpanRecord, SpanStart, TraceDetail,
    TraceStart, TraceStatus, TraceSummary,
};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS at_experiments (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS at_traces (
  id TEXT PRIMARY KEY NOT NULL,
  experiment_id TEXT NOT NULL REFERENCES at_experiments(id),
  name TEXT NOT NULL,
  session_id TEXT,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  tags_json TEXT NOT NULL DEFAULT '{}',
  FOREIGN KEY (experiment_id) REFERENCES at_experiments(id)
);

CREATE INDEX IF NOT EXISTS idx_at_traces_session ON at_traces(session_id);
CREATE INDEX IF NOT EXISTS idx_at_traces_started ON at_traces(started_at DESC);

CREATE TABLE IF NOT EXISTS at_spans (
  id TEXT PRIMARY KEY NOT NULL,
  trace_id TEXT NOT NULL REFERENCES at_traces(id) ON DELETE CASCADE,
  parent_span_id TEXT,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  inputs_json TEXT,
  outputs_json TEXT,
  metrics_json TEXT NOT NULL DEFAULT '{}',
  attributes_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_at_spans_trace ON at_spans(trace_id);
"#;

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

fn parse_map_string(raw: &str) -> BTreeMap<String, String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn tokens_from_metrics(metrics: &BTreeMap<String, f64>) -> f64 {
    if let Some(total) = metrics.get("total_tokens") {
        return (*total).max(0.0);
    }
    metrics.get("input_tokens").copied().unwrap_or(0.0).max(0.0)
        + metrics
            .get("output_tokens")
            .copied()
            .unwrap_or(0.0)
            .max(0.0)
}

fn optional_token_total(raw: f64) -> Option<i64> {
    let rounded = raw.round() as i64;
    if rounded > 0 {
        Some(rounded)
    } else {
        None
    }
}

/// Sums token metrics across spans for list queries.
const TOTAL_TOKENS_SUBQUERY: &str = r#"
(SELECT COALESCE(SUM(
  CASE
    WHEN json_extract(s.metrics_json, '$.total_tokens') IS NOT NULL
      THEN COALESCE(json_extract(s.metrics_json, '$.total_tokens'), 0)
    ELSE COALESCE(json_extract(s.metrics_json, '$.input_tokens'), 0)
       + COALESCE(json_extract(s.metrics_json, '$.output_tokens'), 0)
  END
), 0) FROM at_spans s WHERE s.trace_id = t.id)
"#;

fn parse_map_f64(raw: &str) -> BTreeMap<String, f64> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn latency_ms(started_at: &str, ended_at: Option<&str>) -> Option<i64> {
    let ended = ended_at?;
    let start = OffsetDateTime::parse(started_at, &Rfc3339).ok()?;
    let end = OffsetDateTime::parse(ended, &Rfc3339).ok()?;
    let ms = (end - start).whole_milliseconds();
    Some(ms.max(0) as i64)
}

/// Soft AgentTrace handle — write failures are logged, never panic callers.
#[derive(Clone)]
pub struct AgentTrace {
    pool: SqlitePool,
    path: PathBuf,
}

impl AgentTrace {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::query(SCHEMA).execute(&pool).await?;
        Ok(Self { pool, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn experiment(&self, name: &str) -> Result<ExperimentHandle> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AgentTraceError::msg("experiment name is required"));
        }
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM at_experiments WHERE name = ?")
                .bind(name)
                .fetch_optional(&self.pool)
                .await?;
        let id = if let Some((id,)) = existing {
            id
        } else {
            let id = Uuid::new_v4().to_string();
            let created = now_rfc3339();
            sqlx::query(
                "INSERT INTO at_experiments (id, name, created_at) VALUES (?, ?, ?)",
            )
            .bind(&id)
            .bind(name)
            .bind(&created)
            .execute(&self.pool)
            .await?;
            id
        };
        Ok(ExperimentHandle {
            flow: self.clone(),
            experiment_id: id,
            experiment_name: name.to_string(),
        })
    }

    pub async fn list_experiments(&self) -> Result<Vec<ExperimentRecord>> {
        let rows = sqlx::query(
            "SELECT id, name, created_at FROM at_experiments ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ExperimentRecord {
                id: row.get("id"),
                name: row.get("name"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn list_traces(&self, filter: ListTracesFilter) -> Result<Vec<TraceSummary>> {
        let limit = filter.limit.unwrap_or(100).clamp(1, 500) as i64;
        let token_sql = TOTAL_TOKENS_SUBQUERY;
        let rows = if let Some(session) = filter.session_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(exp) = filter.experiment.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                let q = format!(
                    r#"
                    SELECT t.id, t.experiment_id, e.name AS experiment_name, t.name, t.session_id,
                           t.status, t.started_at, t.ended_at, t.tags_json,
                           (SELECT COUNT(*) FROM at_spans s WHERE s.trace_id = t.id) AS span_count,
                           {token_sql} AS total_tokens
                    FROM at_traces t
                    JOIN at_experiments e ON e.id = t.experiment_id
                    WHERE t.session_id = ? AND e.name = ?
                    ORDER BY t.started_at DESC
                    LIMIT ?
                    "#
                );
                sqlx::query(&q)
                    .bind(session)
                    .bind(exp)
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await?
            } else {
                let q = format!(
                    r#"
                    SELECT t.id, t.experiment_id, e.name AS experiment_name, t.name, t.session_id,
                           t.status, t.started_at, t.ended_at, t.tags_json,
                           (SELECT COUNT(*) FROM at_spans s WHERE s.trace_id = t.id) AS span_count,
                           {token_sql} AS total_tokens
                    FROM at_traces t
                    JOIN at_experiments e ON e.id = t.experiment_id
                    WHERE t.session_id = ?
                    ORDER BY t.started_at DESC
                    LIMIT ?
                    "#
                );
                sqlx::query(&q)
                    .bind(session)
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await?
            }
        } else if let Some(exp) = filter.experiment.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let q = format!(
                r#"
                SELECT t.id, t.experiment_id, e.name AS experiment_name, t.name, t.session_id,
                       t.status, t.started_at, t.ended_at, t.tags_json,
                       (SELECT COUNT(*) FROM at_spans s WHERE s.trace_id = t.id) AS span_count,
                       {token_sql} AS total_tokens
                FROM at_traces t
                JOIN at_experiments e ON e.id = t.experiment_id
                WHERE e.name = ?
                ORDER BY t.started_at DESC
                LIMIT ?
                "#
            );
            sqlx::query(&q)
                .bind(exp)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
        } else {
            let q = format!(
                r#"
                SELECT t.id, t.experiment_id, e.name AS experiment_name, t.name, t.session_id,
                       t.status, t.started_at, t.ended_at, t.tags_json,
                       (SELECT COUNT(*) FROM at_spans s WHERE s.trace_id = t.id) AS span_count,
                       {token_sql} AS total_tokens
                FROM at_traces t
                JOIN at_experiments e ON e.id = t.experiment_id
                ORDER BY t.started_at DESC
                LIMIT ?
                "#
            );
            sqlx::query(&q)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
        };

        Ok(rows
            .into_iter()
            .map(|row| {
                let started_at: String = row.get("started_at");
                let ended_at: Option<String> = row.get("ended_at");
                let tags_json: String = row.get("tags_json");
                let total_tokens: f64 = row.try_get("total_tokens").unwrap_or(0.0);
                TraceSummary {
                    id: row.get("id"),
                    experiment_id: row.get("experiment_id"),
                    experiment_name: row.get("experiment_name"),
                    name: row.get("name"),
                    session_id: row.get("session_id"),
                    status: row.get("status"),
                    latency_ms: latency_ms(&started_at, ended_at.as_deref()),
                    started_at,
                    ended_at,
                    span_count: row.get("span_count"),
                    total_tokens: optional_token_total(total_tokens),
                    tags: parse_map_string(&tags_json),
                }
            })
            .collect())
    }

    pub async fn list_sessions(&self, experiment: Option<&str>, limit: usize) -> Result<Vec<SessionSummary>> {
        let limit = limit.clamp(1, 500) as i64;
        let rows = if let Some(exp) = experiment.map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(
                r#"
                SELECT t.session_id AS session_id,
                       COUNT(*) AS trace_count,
                       MAX(t.started_at) AS last_at,
                       MIN(t.started_at) AS first_at
                FROM at_traces t
                JOIN at_experiments e ON e.id = t.experiment_id
                WHERE t.session_id IS NOT NULL AND t.session_id != '' AND e.name = ?
                GROUP BY t.session_id
                ORDER BY last_at DESC
                LIMIT ?
                "#,
            )
            .bind(exp)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT t.session_id AS session_id,
                       COUNT(*) AS trace_count,
                       MAX(t.started_at) AS last_at,
                       MIN(t.started_at) AS first_at
                FROM at_traces t
                WHERE t.session_id IS NOT NULL AND t.session_id != ''
                GROUP BY t.session_id
                ORDER BY last_at DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows
            .into_iter()
            .map(|row| SessionSummary {
                session_id: row.get("session_id"),
                trace_count: row.get::<i64, _>("trace_count") as usize,
                first_at: row.get("first_at"),
                last_at: row.get("last_at"),
            })
            .collect())
    }

    pub async fn get_trace(&self, trace_id: &str) -> Result<Option<TraceDetail>> {
        let trace_id = trace_id.trim();
        if trace_id.is_empty() {
            return Ok(None);
        }
        let row = sqlx::query(
            r#"
            SELECT t.id, t.experiment_id, e.name AS experiment_name, t.name, t.session_id,
                   t.status, t.started_at, t.ended_at, t.tags_json,
                   (SELECT COUNT(*) FROM at_spans s WHERE s.trace_id = t.id) AS span_count
            FROM at_traces t
            JOIN at_experiments e ON e.id = t.experiment_id
            WHERE t.id = ?
            "#,
        )
        .bind(trace_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let started_at: String = row.get("started_at");
        let ended_at: Option<String> = row.get("ended_at");
        let tags_json: String = row.get("tags_json");
        let span_rows = sqlx::query(
            r#"
            SELECT id, trace_id, parent_span_id, name, kind, status, started_at, ended_at,
                   inputs_json, outputs_json, metrics_json, attributes_json
            FROM at_spans
            WHERE trace_id = ?
            ORDER BY started_at ASC
            "#,
        )
        .bind(trace_id)
        .fetch_all(&self.pool)
        .await?;
        let spans: Vec<SpanRecord> = span_rows
            .into_iter()
            .map(|row| {
                let started_at: String = row.get("started_at");
                let ended_at: Option<String> = row.get("ended_at");
                let inputs_json: Option<String> = row.get("inputs_json");
                let outputs_json: Option<String> = row.get("outputs_json");
                let metrics_json: String = row.get("metrics_json");
                let attributes_json: String = row.get("attributes_json");
                SpanRecord {
                    id: row.get("id"),
                    trace_id: row.get("trace_id"),
                    parent_span_id: row.get("parent_span_id"),
                    name: row.get("name"),
                    kind: row.get("kind"),
                    status: row.get("status"),
                    latency_ms: latency_ms(&started_at, ended_at.as_deref()),
                    started_at,
                    ended_at,
                    inputs: inputs_json.and_then(|s| serde_json::from_str(&s).ok()),
                    outputs: outputs_json.and_then(|s| serde_json::from_str(&s).ok()),
                    metrics: parse_map_f64(&metrics_json),
                    attributes: parse_map_string(&attributes_json),
                }
            })
            .collect();
        let total_tokens = optional_token_total(
            spans
                .iter()
                .map(|span| tokens_from_metrics(&span.metrics))
                .sum(),
        );
        let summary = TraceSummary {
            id: row.get("id"),
            experiment_id: row.get("experiment_id"),
            experiment_name: row.get("experiment_name"),
            name: row.get("name"),
            session_id: row.get("session_id"),
            status: row.get("status"),
            latency_ms: latency_ms(&started_at, ended_at.as_deref()),
            started_at,
            ended_at,
            span_count: spans.len() as i64,
            total_tokens,
            tags: parse_map_string(&tags_json),
        };
        Ok(Some(TraceDetail {
            trace: summary,
            spans,
        }))
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<u64> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query(
            r#"
            DELETE FROM at_traces
            WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_trace(&self, trace_id: &str) -> Result<u64> {
        let trace_id = trace_id.trim();
        if trace_id.is_empty() {
            return Ok(0);
        }
        let result = sqlx::query("DELETE FROM at_traces WHERE id = ?")
            .bind(trace_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub trace_count: usize,
    pub first_at: Option<String>,
    pub last_at: Option<String>,
}

#[derive(Clone)]
pub struct ExperimentHandle {
    flow: AgentTrace,
    experiment_id: String,
    experiment_name: String,
}

impl ExperimentHandle {
    pub fn id(&self) -> &str {
        &self.experiment_id
    }

    pub fn name(&self) -> &str {
        &self.experiment_name
    }

    pub async fn start_trace(&self, start: TraceStart) -> Result<TraceHandle> {
        let id = Uuid::new_v4().to_string();
        let started = now_rfc3339();
        let tags = serde_json::to_string(&start.tags).unwrap_or_else(|_| "{}".into());
        let session = start
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        sqlx::query(
            r#"
            INSERT INTO at_traces (id, experiment_id, name, session_id, status, started_at, tags_json)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&self.experiment_id)
        .bind(start.name.trim())
        .bind(&session)
        .bind(TraceStatus::Running.as_str())
        .bind(&started)
        .bind(&tags)
        .execute(&self.flow.pool)
        .await?;
        Ok(TraceHandle {
            flow: self.flow.clone(),
            trace_id: id,
            session_id: session,
        })
    }
}

#[derive(Clone)]
pub struct TraceHandle {
    flow: AgentTrace,
    trace_id: String,
    session_id: Option<String>,
}

impl TraceHandle {
    pub fn id(&self) -> &str {
        &self.trace_id
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub async fn span(&self, start: SpanStart) -> Result<SpanHandle> {
        let id = Uuid::new_v4().to_string();
        let started = now_rfc3339();
        let inputs = start
            .inputs
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok());
        let attrs = serde_json::to_string(&start.attributes).unwrap_or_else(|_| "{}".into());
        sqlx::query(
            r#"
            INSERT INTO at_spans (
              id, trace_id, parent_span_id, name, kind, status, started_at,
              inputs_json, metrics_json, attributes_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, '{}', ?)
            "#,
        )
        .bind(&id)
        .bind(&self.trace_id)
        .bind(&start.parent_span_id)
        .bind(start.name.trim())
        .bind(start.kind.as_str())
        .bind(TraceStatus::Running.as_str())
        .bind(&started)
        .bind(&inputs)
        .bind(&attrs)
        .execute(&self.flow.pool)
        .await?;
        Ok(SpanHandle {
            flow: self.flow.clone(),
            span_id: id,
            trace_id: self.trace_id.clone(),
        })
    }

    pub async fn end(&self, status: TraceStatus) -> Result<()> {
        let ended = now_rfc3339();
        sqlx::query("UPDATE at_traces SET status = ?, ended_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(&ended)
            .bind(&self.trace_id)
            .execute(&self.flow.pool)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct SpanHandle {
    flow: AgentTrace,
    span_id: String,
    trace_id: String,
}

impl SpanHandle {
    pub fn id(&self) -> &str {
        &self.span_id
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub async fn end(&self, end: SpanEnd) -> Result<()> {
        let ended = now_rfc3339();
        let inputs = end
            .inputs
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok());
        let outputs = end
            .outputs
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok());
        let metrics = serde_json::to_string(&end.metrics).unwrap_or_else(|_| "{}".into());
        // Merge attributes: load existing then extend
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT attributes_json FROM at_spans WHERE id = ?")
                .bind(&self.span_id)
                .fetch_optional(&self.flow.pool)
                .await?;
        let mut attrs = existing
            .as_ref()
            .map(|r| parse_map_string(&r.0))
            .unwrap_or_default();
        attrs.extend(end.attributes);
        let attrs_json = serde_json::to_string(&attrs).unwrap_or_else(|_| "{}".into());
        if let Some(inputs_json) = inputs {
            sqlx::query(
                r#"
                UPDATE at_spans
                SET status = ?, ended_at = ?, inputs_json = ?, outputs_json = ?,
                    metrics_json = ?, attributes_json = ?
                WHERE id = ?
                "#,
            )
            .bind(end.status.as_str())
            .bind(&ended)
            .bind(&inputs_json)
            .bind(&outputs)
            .bind(&metrics)
            .bind(&attrs_json)
            .bind(&self.span_id)
            .execute(&self.flow.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE at_spans
                SET status = ?, ended_at = ?, outputs_json = ?, metrics_json = ?, attributes_json = ?
                WHERE id = ?
                "#,
            )
            .bind(end.status.as_str())
            .bind(&ended)
            .bind(&outputs)
            .bind(&metrics)
            .bind(&attrs_json)
            .bind(&self.span_id)
            .execute(&self.flow.pool)
            .await?;
        }
        Ok(())
    }
}

/// Soft helpers — never fail the agent turn.
pub async fn soft_end_span(span: Option<&SpanHandle>, end: SpanEnd) {
    let Some(span) = span else { return };
    if let Err(err) = span.end(end).await {
        warn!(error = %err, span_id = %span.id(), "agenttrace span end failed");
    }
}

pub async fn soft_end_trace(trace: Option<&TraceHandle>, status: TraceStatus) {
    let Some(trace) = trace else { return };
    if let Err(err) = trace.end(status).await {
        warn!(error = %err, trace_id = %trace.id(), "agenttrace trace end failed");
    }
}

pub async fn soft_start_span(trace: Option<&TraceHandle>, start: SpanStart) -> Option<SpanHandle> {
    let Some(trace) = trace else { return None };
    match trace.span(start).await {
        Ok(span) => Some(span),
        Err(err) => {
            warn!(error = %err, "agenttrace span start failed");
            None
        }
    }
}

pub type SharedAgentTrace = Arc<AgentTrace>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SpanKind;

    #[tokio::test]
    async fn start_end_list_get_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agenttrace.db");
        let flow = AgentTrace::open(&path).await.unwrap();
        let exp = flow.experiment("yazg").await.unwrap();
        let mut tags = BTreeMap::new();
        tags.insert("agent".into(), "yazg".into());
        let trace = exp
            .start_trace(TraceStart {
                name: "turn".into(),
                session_id: Some("yazg-chat:abc".into()),
                tags,
            })
            .await
            .unwrap();
        let classify = crate::start_span!(
            Some(&trace),
            SpanStart {
                name: String::new(),
                kind: SpanKind::Capability,
                parent_span_id: None,
                inputs: Some(serde_json::json!({"user":"hi"})),
                attributes: BTreeMap::new(),
            }
        )
        .await
        .unwrap();
        soft_end_span(
            Some(&classify),
            SpanEnd {
                inputs: None,
                outputs: Some(serde_json::json!({"capability":"conversation"})),
                status: TraceStatus::Ok,
                metrics: BTreeMap::from([("latency_ms".into(), 12.0)]),
                attributes: BTreeMap::new(),
            },
        )
        .await;
        let llm = crate::start_span!(
            Some(&trace),
            SpanStart {
                // Explicit override (closures / multi-span sites).
                name: "llm_wire".into(),
                kind: SpanKind::Llm,
                parent_span_id: None,
                inputs: Some(serde_json::json!({"messages":[]})),
                attributes: BTreeMap::new(),
            }
        )
        .await
        .unwrap();
        soft_end_span(
            Some(&llm),
            SpanEnd {
                inputs: None,
                outputs: Some(serde_json::json!({"content":"hello"})),
                status: TraceStatus::Ok,
                metrics: BTreeMap::new(),
                attributes: BTreeMap::new(),
            },
        )
        .await;
        soft_end_trace(Some(&trace), TraceStatus::Ok).await;

        let listed = flow
            .list_traces(ListTracesFilter {
                experiment: Some("yazg".into()),
                session_id: Some("yazg-chat:abc".into()),
                limit: Some(10),
            })
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].span_count, 2);

        let detail = flow.get_trace(&listed[0].id).await.unwrap().unwrap();
        assert_eq!(detail.spans.len(), 2);
        assert_eq!(detail.spans[0].name, "start_end_list_get_delete");
        assert_eq!(detail.spans[1].name, "llm_wire");

        let sessions = flow.list_sessions(Some("yazg"), 10).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].trace_count, 1);

        let deleted = flow.delete_session("yazg-chat:abc").await.unwrap();
        assert_eq!(deleted, 1);
        assert!(flow
            .list_traces(ListTracesFilter {
                session_id: Some("yazg-chat:abc".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .is_empty());
    }
}
