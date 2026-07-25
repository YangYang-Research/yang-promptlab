use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use promptlab_core::{PromptLabError, PromptLabResult};
use time::{Duration, OffsetDateTime};
use tracing::{debug, instrument};
use url::Url;

use crate::config::AuthEngineConfig;
use crate::mock::SharedPlaywrightDriver;
use crate::playwright::{PlaywrightClient, PlaywrightDriver};
use crate::session::SessionStore;
use crate::types::{
    AuthSession, CookieRecord, ExtractedToken, PlaywrightStorageState, SessionValidationStatus,
};

/// Loaded browser session material used by discovery and attack engines.
#[derive(Debug, Clone)]
pub struct SessionAuthContext {
    pub session_id: String,
    pub cookies: Vec<CookieRecord>,
    pub tokens: Vec<ExtractedToken>,
    pub storage_state_path: Option<PathBuf>,
    pub validation_status: SessionValidationStatus,
    pub user_identity: Option<String>,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub last_validated_at: Option<OffsetDateTime>,
}

/// Manages Playwright storageState, session validation, and auth material for downstream engines.
pub struct AuthSessionManager {
    store: SessionStore,
    driver: SharedPlaywrightDriver,
    config: AuthEngineConfig,
}

impl AuthSessionManager {
    pub async fn new(
        store: SessionStore,
        config: AuthEngineConfig,
        driver: Option<SharedPlaywrightDriver>,
    ) -> PromptLabResult<Self> {
        let driver = match driver {
            Some(d) => d,
            None => Arc::new(PlaywrightClient::new(config.clone()).await?),
        };
        Ok(Self {
            store,
            driver,
            config,
        })
    }

    pub fn store(&self) -> &SessionStore {
        &self.store
    }

    pub fn config(&self) -> &AuthEngineConfig {
        &self.config
    }

    pub fn driver(&self) -> &SharedPlaywrightDriver {
        &self.driver
    }

    /// Load session cookies, tokens, and storageState path from the database.
    pub async fn load_context(&self, session_id: &str) -> PromptLabResult<SessionAuthContext> {
        let session = self.store.get_session(session_id).await?;
        Ok(session_to_context(session))
    }

    /// Load Playwright storageState JSON from disk.
    pub async fn load_storage_state(
        &self,
        session_id: &str,
    ) -> PromptLabResult<Option<PlaywrightStorageState>> {
        let session = self.store.get_session(session_id).await?;
        let Some(path) = session.storage_state_path else {
            return Ok(None);
        };
        let state = self
            .store
            .load_storage_state(Path::new(&path))
            .await?;
        Ok(Some(state))
    }

    /// Playwright browser context options for authenticated discovery/attack.
    pub async fn browser_context_options(
        &self,
        session_id: &str,
    ) -> PromptLabResult<Option<PathBuf>> {
        let session = self.store.get_session(session_id).await?;
        Ok(session
            .storage_state_path
            .map(PathBuf::from))
    }

    /// Validate session expiry and optionally probe the target URL with stored cookies.
    #[instrument(skip(self))]
    pub async fn validate_session(
        &self,
        session_id: &str,
        probe_url: Option<&str>,
    ) -> PromptLabResult<SessionAuthContext> {
        let mut ctx = self.load_context(session_id).await?;
        let computed = compute_validation_status(&ctx.cookies, ctx.expires_at);

        let mut status = computed;
        if status != SessionValidationStatus::Expired {
            if let Some(url) = probe_url {
                if let Some(cookie_header) = cookie_header_for_url(&ctx.cookies, url) {
                    if probe_requires_reauth(url, &cookie_header).await {
                        status = SessionValidationStatus::Expired;
                    }
                }
            }
        }

        let now = OffsetDateTime::now_utc();
        ctx.validation_status = status;
        ctx.last_validated_at = Some(now);

        self.store
            .update_validation(
                session_id,
                status,
                Some(now),
                ctx.user_identity.as_deref(),
                ctx.expires_at,
            )
            .await?;

        debug!(%session_id, status = %status.as_str(), "session validated");
        Ok(ctx)
    }

    /// Recompute expiry metadata from cookies/tokens and persist it.
    pub async fn refresh_metadata(&self, session_id: &str) -> PromptLabResult<SessionAuthContext> {
        let session = self.store.get_session(session_id).await?;
        let expires_at = earliest_cookie_expiry(&session.cookies).or(session.expires_at);
        let user_identity = session
            .user_identity
            .clone()
            .or_else(|| infer_user_identity(&session.tokens, &session.cookies));

        self.store
            .update_validation(
                session_id,
                session.validation_status,
                session.last_validated_at,
                user_identity.as_deref(),
                expires_at,
            )
            .await?;

        let updated = self.store.get_session(session_id).await?;
        Ok(session_to_context(updated))
    }

    /// HTTP headers (Authorization, API keys, Cookie) derived from the session.
    pub fn auth_headers(ctx: &SessionAuthContext) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        for token in &ctx.tokens {
            if let Some(name) = token.header_name.as_deref() {
                if !name.trim().is_empty() && !token.value.trim().is_empty() {
                    headers.insert(name.to_string(), token.value.trim().to_string());
                }
            }
        }
        headers
    }

    pub fn cookie_header_for_url(ctx: &SessionAuthContext, url: &str) -> Option<String> {
        cookie_header_for_url(&ctx.cookies, url)
    }
}

fn session_to_context(session: AuthSession) -> SessionAuthContext {
    SessionAuthContext {
        session_id: session.id,
        cookies: session.cookies,
        tokens: session.tokens,
        storage_state_path: session
            .storage_state_path
            .map(PathBuf::from),
        validation_status: session.validation_status,
        user_identity: session.user_identity,
        created_at: session.created_at,
        expires_at: session.expires_at,
        last_validated_at: session.last_validated_at,
    }
}

pub fn compute_validation_status(
    cookies: &[CookieRecord],
    expires_at: Option<OffsetDateTime>,
) -> SessionValidationStatus {
    let now = OffsetDateTime::now_utc();
    let soon = now + Duration::hours(24);

    if let Some(exp) = expires_at {
        if exp <= now {
            return SessionValidationStatus::Expired;
        }
        if exp <= soon {
            return SessionValidationStatus::ExpiringSoon;
        }
    }

    let mut has_expiry = false;
    let mut earliest: Option<OffsetDateTime> = None;

    for cookie in cookies {
        if let Some(raw) = cookie.expires {
            has_expiry = true;
            let exp = cookie_expiry(raw);
            earliest = Some(match earliest {
                Some(current) if current < exp => current,
                _ => exp,
            });
        }
    }

    if let Some(exp) = earliest {
        if exp <= now {
            return SessionValidationStatus::Expired;
        }
        if exp <= soon {
            return SessionValidationStatus::ExpiringSoon;
        }
    }

    if has_expiry || expires_at.is_some() {
        SessionValidationStatus::Valid
    } else {
        SessionValidationStatus::Valid
    }
}

fn cookie_expiry(raw: f64) -> OffsetDateTime {
    let secs = raw as i64;
    OffsetDateTime::from_unix_timestamp(secs).unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

pub fn earliest_cookie_expiry(cookies: &[CookieRecord]) -> Option<OffsetDateTime> {
    cookies
        .iter()
        .filter_map(|c| c.expires.map(cookie_expiry))
        .min()
}

pub fn infer_user_identity(
    tokens: &[ExtractedToken],
    cookies: &[CookieRecord],
) -> Option<String> {
    for token in tokens {
        if token.kind.contains("id") || token.kind == "oidc_id" {
            return Some(mask_token(&token.value));
        }
    }
    for cookie in cookies {
        if cookie.name.contains("user") || cookie.name == "email" {
            return Some(mask_token(&cookie.value));
        }
    }
    None
}

fn mask_token(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 8 {
        return trimmed.to_string();
    }
    format!("{}…{}", &trimmed[..4], &trimmed[trimmed.len() - 4..])
}

pub fn cookie_header_for_url(cookies: &[CookieRecord], url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    let path = parsed.path();

    let pairs: Vec<String> = cookies
        .iter()
        .filter(|cookie| cookie_matches_url(cookie, host, path))
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect();

    if pairs.is_empty() {
        None
    } else {
        Some(pairs.join("; "))
    }
}

fn cookie_matches_url(cookie: &CookieRecord, host: &str, path: &str) -> bool {
    let domain = cookie.domain.trim_start_matches('.');
    let host_match = host == domain || host.ends_with(&format!(".{domain}"));
    if !host_match {
        return false;
    }
    path.starts_with(&cookie.path)
}

async fn probe_requires_reauth(url: &str, cookie_header: &str) -> bool {
    let client = match promptlab_core::build_http_client(
        promptlab_core::HttpClientOptions::default()
            .with_redirect_limit(3)
            .with_timeout(std::time::Duration::from_secs(10)),
    ) {
        Ok(client) => client,
        Err(_) => return false,
    };

    let response = client
        .get(url)
        .header("Cookie", cookie_header)
        .send()
        .await;

    match response {
        Ok(resp) => matches!(resp.status().as_u16(), 401 | 403),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_cookie_marks_session_expired() {
        let past = (OffsetDateTime::now_utc() - Duration::hours(1)).unix_timestamp() as f64;
        let cookies = vec![CookieRecord {
            name: "sid".into(),
            value: "abc".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expires: Some(past),
            http_only: false,
            secure: false,
            same_site: None,
        }];
        assert_eq!(
            compute_validation_status(&cookies, None),
            SessionValidationStatus::Expired
        );
    }

    #[test]
    fn cookie_header_filters_by_domain() {
        let cookies = vec![CookieRecord {
            name: "sid".into(),
            value: "abc".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expires: None,
            http_only: false,
            secure: false,
            same_site: None,
        }];
        let header = cookie_header_for_url(&cookies, "https://example.com/app").unwrap();
        assert_eq!(header, "sid=abc");
    }
}
