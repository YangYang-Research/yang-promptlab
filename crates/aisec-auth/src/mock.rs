use std::sync::Arc;

use async_trait::async_trait;
use aisec_core::AisecResult;
use serde_json::json;

use crate::playwright::{PlaywrightDriver, RecordLoginResult, ReplaySessionResult};
use crate::types::{CookieRecord, ExtractedToken, RecordLoginOptions, ReplayOptions};

/// In-memory Playwright driver for unit tests (no Node.js required).
pub struct MockPlaywrightDriver {
    pub record_result: RecordLoginResult,
    pub replay_result: ReplaySessionResult,
    pub cookies: Vec<CookieRecord>,
    pub tokens: Vec<ExtractedToken>,
}

impl MockPlaywrightDriver {
    pub fn login_success() -> Self {
        Self {
            record_result: RecordLoginResult {
                steps: vec![json!({"action":"fill","selector":"#user"})],
                storage_state: json!({"cookies":[{"name":"sid","value":"abc","domain":"example.com","path":"/"}],"origins":[]}),
                cookies: vec![json!({"name":"sid","value":"abc","domain":"example.com","path":"/"})],
                tokens: vec![json!({"kind":"bearer","source":"response_header","value":"Bearer tok123"})],
                final_url: "https://example.com/dashboard".into(),
            },
            replay_result: ReplaySessionResult {
                url: "https://example.com/dashboard".into(),
                cookies: vec![json!({"name":"sid","value":"abc","domain":"example.com","path":"/"})],
                tokens: vec![],
            },
            cookies: vec![CookieRecord {
                name: "sid".into(),
                value: "abc".into(),
                domain: "example.com".into(),
                path: "/".into(),
                expires: None,
                http_only: true,
                secure: true,
                same_site: None,
            }],
            tokens: vec![ExtractedToken {
                kind: "bearer".into(),
                source: "response_header".into(),
                value: "Bearer tok123".into(),
                url: Some("https://example.com/oauth/token".into()),
                header_name: Some("Authorization".into()),
            }],
        }
    }
}

#[async_trait]
impl PlaywrightDriver for MockPlaywrightDriver {
    async fn launch(&self, _options: crate::playwright::PlaywrightOptions) -> AisecResult<()> {
        Ok(())
    }

    async fn close(&self) -> AisecResult<()> {
        Ok(())
    }

    async fn record_login(
        &self,
        _url: &str,
        _method: &str,
        _config: serde_json::Value,
        _options: RecordLoginOptions,
    ) -> AisecResult<RecordLoginResult> {
        Ok(self.record_result.clone())
    }

    async fn replay_session(
        &self,
        _url: &str,
        _storage_state: Option<serde_json::Value>,
        _storage_state_path: Option<&std::path::Path>,
        _options: ReplayOptions,
    ) -> AisecResult<ReplaySessionResult> {
        Ok(self.replay_result.clone())
    }

    async fn extract_tokens(&self, _url: Option<&str>) -> AisecResult<Vec<ExtractedToken>> {
        Ok(self.tokens.clone())
    }

    async fn get_cookies(&self, _url: Option<&str>) -> AisecResult<Vec<CookieRecord>> {
        Ok(self.cookies.clone())
    }

    async fn set_cookies(&self, cookies: Vec<CookieRecord>) -> AisecResult<Vec<CookieRecord>> {
        Ok(cookies)
    }
}

pub type SharedPlaywrightDriver = Arc<dyn PlaywrightDriver>;
