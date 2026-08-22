use std::sync::Arc;

use async_trait::async_trait;
use promptlab_core::PromptLabResult;
use serde_json::json;

use crate::playwright::{ExecuteHttpResult, PlaywrightDriver, RecordLoginResult, ReplaySessionResult};
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
    async fn launch(&self, _options: crate::playwright::PlaywrightOptions) -> PromptLabResult<()> {
        Ok(())
    }

    async fn close(&self) -> PromptLabResult<()> {
        Ok(())
    }

    async fn record_login(
        &self,
        _url: &str,
        _method: &str,
        _config: serde_json::Value,
        _options: RecordLoginOptions,
    ) -> PromptLabResult<RecordLoginResult> {
        Ok(self.record_result.clone())
    }

    async fn begin_interactive_login(
        &self,
        _url: &str,
        _options: RecordLoginOptions,
    ) -> PromptLabResult<()> {
        Ok(())
    }

    async fn finish_interactive_login(&self) -> PromptLabResult<RecordLoginResult> {
        Ok(self.record_result.clone())
    }

    async fn replay_session(
        &self,
        _url: &str,
        _storage_state: Option<serde_json::Value>,
        _storage_state_path: Option<&std::path::Path>,
        _options: ReplayOptions,
    ) -> PromptLabResult<ReplaySessionResult> {
        Ok(self.replay_result.clone())
    }

    async fn extract_tokens(&self, _url: Option<&str>) -> PromptLabResult<Vec<ExtractedToken>> {
        Ok(self.tokens.clone())
    }

    async fn get_cookies(&self, _url: Option<&str>) -> PromptLabResult<Vec<CookieRecord>> {
        Ok(self.cookies.clone())
    }

    async fn set_cookies(&self, cookies: Vec<CookieRecord>) -> PromptLabResult<Vec<CookieRecord>> {
        Ok(cookies)
    }

    async fn execute_http_request(
        &self,
        _url: &str,
        _method: &str,
        _headers: std::collections::HashMap<String, String>,
        _body: Option<String>,
        _storage_state_path: Option<&std::path::Path>,
    ) -> PromptLabResult<ExecuteHttpResult> {
        Ok(ExecuteHttpResult {
            status: 200,
            headers: std::collections::HashMap::new(),
            body: String::new(),
            duration_ms: 0,
        })
    }

    async fn send_chat_prompt_ex(
        &self,
        args: crate::playwright::ChatPromptArgs<'_>,
    ) -> PromptLabResult<String> {
        Ok(format!("mock-response:{}", args.prompt))
    }
}

pub type SharedPlaywrightDriver = Arc<dyn PlaywrightDriver>;
