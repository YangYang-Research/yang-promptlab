//! Workspace search for header routing — metadata only, no evidence/payloads.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::StorageResultExt;
use promptlab_core::PromptLabResult;

pub const WORKSPACE_SEARCH_PER_KIND: i64 = 4;
pub const WORKSPACE_SEARCH_MAX_QUERY_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchHit {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub to: String,
}

#[derive(sqlx::FromRow)]
struct HitRow {
    id: String,
    title: String,
    subtitle: String,
}

/// Search projects/targets/scans/findings/reports/techniques. Empty query → no hits.
/// Selected columns are id + display fields only (no JSON blobs or prompt bodies).
pub async fn search_workspace(pool: &SqlitePool, raw: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let query = raw.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let query = clip_query(query);

    let like = like_pattern(query);
    let projects = search_projects(pool, &like).await?;
    let targets = search_targets(pool, &like).await?;
    let scans = search_scans(pool, &like).await?;
    let findings = search_findings(pool, query, &like).await?;
    let reports = search_reports(pool, &like).await?;
    let techniques = search_techniques(pool, &like).await?;

    let mut hits = Vec::with_capacity(
        projects.len()
            + targets.len()
            + scans.len()
            + findings.len()
            + reports.len()
            + techniques.len(),
    );
    hits.extend(projects);
    hits.extend(targets);
    hits.extend(scans);
    hits.extend(findings);
    hits.extend(reports);
    hits.extend(techniques);
    Ok(hits)
}

pub fn like_pattern(query: &str) -> String {
    let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

fn clip_query(query: &str) -> &str {
    if query.len() <= WORKSPACE_SEARCH_MAX_QUERY_CHARS {
        return query;
    }
    let mut end = WORKSPACE_SEARCH_MAX_QUERY_CHARS;
    while end > 0 && !query.is_char_boundary(end) {
        end -= 1;
    }
    &query[..end]
}

fn fts_match_query(raw: &str) -> Option<String> {
    let tokens: Vec<String> = raw
        .split_whitespace()
        .map(|token| token.replace('"', ""))
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{token}\""))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" AND "))
    }
}

fn map_hits(kind: &str, path: &str, rows: Vec<HitRow>) -> Vec<WorkspaceSearchHit> {
    rows.into_iter()
        .map(|row| WorkspaceSearchHit {
            id: format!("{kind}:{}", row.id),
            kind: kind.into(),
            title: row.title,
            subtitle: row.subtitle,
            to: format!("{path}/{}", row.id),
        })
        .collect()
}

async fn search_projects(pool: &SqlitePool, like: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let rows = sqlx::query_as::<_, HitRow>(
        r#"
        SELECT id, name AS title, COALESCE(description, '') AS subtitle
        FROM projects
        WHERE name LIKE ? ESCAPE '\'
           OR IFNULL(description, '') LIKE ? ESCAPE '\'
        ORDER BY updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(like)
    .bind(like)
    .bind(WORKSPACE_SEARCH_PER_KIND)
    .fetch_all(pool)
    .await
    .map_storage()?;
    Ok(map_hits("project", "/projects", rows))
}

async fn search_targets(pool: &SqlitePool, like: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let rows = sqlx::query_as::<_, HitRow>(
        r#"
        SELECT t.id AS id,
               t.name AS title,
               COALESCE(p.name, t.target_type, '') AS subtitle
        FROM targets t
        LEFT JOIN projects p ON p.id = t.project_id
        WHERE t.name LIKE ? ESCAPE '\'
           OR t.target_type LIKE ? ESCAPE '\'
        ORDER BY t.updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(like)
    .bind(like)
    .bind(WORKSPACE_SEARCH_PER_KIND)
    .fetch_all(pool)
    .await
    .map_storage()?;
    Ok(map_hits("target", "/targets", rows))
}

async fn search_scans(pool: &SqlitePool, like: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let rows = sqlx::query_as::<_, HitRow>(
        r#"
        SELECT s.id AS id,
               s.name AS title,
               COALESCE(p.name, s.status, '') AS subtitle
        FROM scans s
        LEFT JOIN projects p ON p.id = s.project_id
        WHERE s.name LIKE ? ESCAPE '\'
           OR s.status LIKE ? ESCAPE '\'
        ORDER BY s.updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(like)
    .bind(like)
    .bind(WORKSPACE_SEARCH_PER_KIND)
    .fetch_all(pool)
    .await
    .map_storage()?;
    Ok(map_hits("scan", "/scans", rows))
}

async fn search_reports(pool: &SqlitePool, like: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let rows = sqlx::query_as::<_, HitRow>(
        r#"
        SELECT r.id AS id,
               r.name AS title,
               COALESCE(p.name, r.format, '') AS subtitle
        FROM reports r
        LEFT JOIN projects p ON p.id = r.project_id
        WHERE r.name LIKE ? ESCAPE '\'
           OR r.format LIKE ? ESCAPE '\'
        ORDER BY r.updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(like)
    .bind(like)
    .bind(WORKSPACE_SEARCH_PER_KIND)
    .fetch_all(pool)
    .await
    .map_storage()?;
    Ok(map_hits("report", "/reports", rows))
}

async fn search_techniques(pool: &SqlitePool, like: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let rows = sqlx::query_as::<_, HitRow>(
        r#"
        SELECT id,
               name AS title,
               COALESCE(NULLIF(owasp, ''), REPLACE(category_id, '_', ' '), '') AS subtitle
        FROM attack_catalog_techniques
        WHERE name LIKE ? ESCAPE '\'
           OR IFNULL(description, '') LIKE ? ESCAPE '\'
           OR category_id LIKE ? ESCAPE '\'
           OR IFNULL(owasp, '') LIKE ? ESCAPE '\'
           OR tags_json LIKE ? ESCAPE '\'
        ORDER BY sort_order ASC, name ASC
        LIMIT ?
        "#,
    )
    .bind(like)
    .bind(like)
    .bind(like)
    .bind(like)
    .bind(like)
    .bind(WORKSPACE_SEARCH_PER_KIND)
    .fetch_all(pool)
    .await
    .map_storage()?;
    Ok(map_hits("technique", "/attack-categories", rows))
}

async fn search_findings(
    pool: &SqlitePool,
    raw: &str,
    like: &str,
) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    if let Some(fts) = fts_match_query(raw) {
        match search_findings_fts(pool, &fts).await {
            Ok(hits) if !hits.is_empty() => return Ok(hits),
            Ok(_) => {}
            Err(err) => {
                tracing::debug!(error = %err, "findings FTS search failed; falling back to LIKE");
            }
        }
    }
    search_findings_like(pool, like).await
}

async fn search_findings_fts(pool: &SqlitePool, fts: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let rows = sqlx::query_as::<_, HitRow>(
        r#"
        SELECT f.id AS id,
               f.title AS title,
               COALESCE(f.category, f.severity, p.name, '') AS subtitle
        FROM findings f
        INNER JOIN findings_fts fts ON f.rowid = fts.rowid
        LEFT JOIN projects p ON p.id = f.project_id
        WHERE findings_fts MATCH ?
        ORDER BY rank
        LIMIT ?
        "#,
    )
    .bind(fts)
    .bind(WORKSPACE_SEARCH_PER_KIND)
    .fetch_all(pool)
    .await
    .map_storage()?;
    Ok(map_hits("finding", "/findings", rows))
}

async fn search_findings_like(pool: &SqlitePool, like: &str) -> PromptLabResult<Vec<WorkspaceSearchHit>> {
    let rows = sqlx::query_as::<_, HitRow>(
        r#"
        SELECT f.id AS id,
               f.title AS title,
               COALESCE(f.category, f.severity, p.name, '') AS subtitle
        FROM findings f
        LEFT JOIN projects p ON p.id = f.project_id
        WHERE f.title LIKE ? ESCAPE '\'
           OR IFNULL(f.category, '') LIKE ? ESCAPE '\'
           OR IFNULL(f.description, '') LIKE ? ESCAPE '\'
        ORDER BY f.updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(like)
    .bind(like)
    .bind(like)
    .bind(WORKSPACE_SEARCH_PER_KIND)
    .fetch_all(pool)
    .await
    .map_storage()?;
    Ok(map_hits("finding", "/findings", rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        CreateFinding, CreateProject, CreateScan, CreateTarget, UpsertAttackCatalogTechnique,
    };
    use crate::pool::test_utils::test_database;
    use crate::repositories::{
        AttackCatalogRepository, FindingRepository, ProjectRepository, ScanRepository,
        TargetRepository,
    };

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("100%_off"), r#"%100\%\_off%"#);
    }

    #[tokio::test]
    async fn search_returns_route_metadata_without_blobs() {
        let db = test_database().await;
        let repos = db.repositories();
        let project = repos
            .projects()
            .create(CreateProject {
                name: "Acme LLM".into(),
                description: Some("prod bot".into()),
            })
            .await
            .unwrap();
        let target = repos
            .targets()
            .create(CreateTarget {
                project_id: project.id.clone(),
                name: "Chat API".into(),
                target_type: "llm".into(),
                descriptor_json: Some(serde_json::json!({
                    "url": "https://secret.example/v1",
                    "auth": {"token": "should-not-appear"}
                })),
                profile_json: None,
            })
            .await
            .unwrap();
        let scan = repos
            .scans()
            .create(CreateScan {
                project_id: project.id.clone(),
                target_id: Some(target.id.clone()),
                name: "Scan (quick)".into(),
                status: Some("completed".into()),
                playbook_json: Some(serde_json::json!({"secret": "nope"})),
            })
            .await
            .unwrap();
        repos
            .findings()
            .create(CreateFinding {
                scan_id: scan.id,
                project_id: project.id,
                target_id: Some(target.id),
                title: "Prompt injection detected".into(),
                severity: "high".into(),
                category: Some("prompt_injection".into()),
                description: Some("leak".into()),
                evidence_json: Some(serde_json::json!({"payload": "huge", "response": "huge"})),
                status: None,
            })
            .await
            .unwrap();
        repos
            .attack_catalog()
            .seed_from(vec![UpsertAttackCatalogTechnique {
                id: "pi-direct-override".into(),
                category_id: "prompt_injection".into(),
                name: "Direct instruction override".into(),
                description: Some("Attempts to override".into()),
                content: "SECRET-BLOB-SHOULD-NOT-APPEAR".into(),
                default_content: "SECRET-BLOB-SHOULD-NOT-APPEAR".into(),
                tags_json: r#"["direct"]"#.into(),
                surface: Some("llm".into()),
                owasp: Some("LLM01".into()),
                enabled: true,
                sort_order: 0,
            }])
            .await
            .unwrap();

        let hits = search_workspace(db.pool(), "acme").await.unwrap();
        let kinds: Vec<_> = hits.iter().map(|hit| hit.kind.as_str()).collect();
        assert!(kinds.contains(&"project"));
        assert!(hits.iter().any(|hit| hit.to.starts_with("/projects/")));
        assert!(hits.iter().all(|hit| {
            !hit.title.contains("huge")
                && !hit.subtitle.contains("huge")
                && !hit.subtitle.contains("should-not-appear")
        }));

        let finding_hits = search_workspace(db.pool(), "injection").await.unwrap();
        assert!(finding_hits.iter().any(|hit| hit.kind == "finding" && hit.to.starts_with("/findings/")));

        let technique_hits = search_workspace(db.pool(), "override").await.unwrap();
        assert!(technique_hits.iter().any(|hit| {
            hit.kind == "technique" && hit.to == "/attack-categories/pi-direct-override"
        }));
        assert!(technique_hits.iter().all(|hit| {
            !hit.title.contains("SECRET-BLOB") && !hit.subtitle.contains("SECRET-BLOB")
        }));
        let content_hits = search_workspace(db.pool(), "SECRET-BLOB").await.unwrap();
        assert!(content_hits.iter().all(|hit| hit.kind != "technique"));
    }

    #[tokio::test]
    async fn percent_in_query_is_literal_not_wildcard() {
        let db = test_database().await;
        let repos = db.repositories();
        repos
            .projects()
            .create(CreateProject {
                name: "100% club".into(),
                description: None,
            })
            .await
            .unwrap();
        repos
            .projects()
            .create(CreateProject {
                name: "other".into(),
                description: None,
            })
            .await
            .unwrap();

        let hits = search_workspace(db.pool(), "100%").await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "100% club");
    }

    #[tokio::test]
    async fn empty_query_returns_nothing() {
        let db = test_database().await;
        assert!(search_workspace(db.pool(), "   ").await.unwrap().is_empty());
    }
}
