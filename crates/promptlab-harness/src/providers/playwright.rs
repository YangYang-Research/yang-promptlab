use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use promptlab_auth::{ChatPromptArgs, SharedPlaywrightDriver};

use crate::error::{HarnessError, HarnessResult};
use crate::models::{AttackRequest, NormalizedResponse};
use crate::traits::Harness;

/// Browser harness — authenticated HTTP replay or deterministic chat UI interaction.
#[derive(Clone)]
pub struct PlaywrightHarness {
    driver: SharedPlaywrightDriver,
    default_headers: HashMap<String, String>,
    storage_state_path: Option<PathBuf>,
    chat_selectors: HashMap<String, String>,
}

impl PlaywrightHarness {
    pub fn new(
        driver: SharedPlaywrightDriver,
        storage_state_path: Option<PathBuf>,
        default_headers: HashMap<String, String>,
        chat_selectors: HashMap<String, String>,
    ) -> Self {
        Self {
            driver,
            default_headers,
            storage_state_path,
            chat_selectors,
        }
    }

    fn has_chat_selectors(&self, request: &AttackRequest) -> bool {
        let selectors = if request.chat_selectors.is_empty() {
            &self.chat_selectors
        } else {
            &request.chat_selectors
        };
        selectors.contains_key("input")
            && selectors.contains_key("submit")
            && selectors.contains_key("response")
    }
}

#[async_trait]
impl Harness for PlaywrightHarness {
    fn id(&self) -> &'static str {
        "playwright"
    }

    async fn execute(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        if self.has_chat_selectors(&request) {
            return self.execute_chat(request).await;
        }
        self.execute_http(request).await
    }
}

impl PlaywrightHarness {
    async fn execute_http(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        let mut headers = self.default_headers.clone();
        for (key, value) in request.merged_headers() {
            headers.insert(key, value);
        }

        let storage_path = request
            .auth
            .storage_state_path
            .as_deref()
            .map(Path::new)
            .or(self.storage_state_path.as_deref());

        let result = self
            .driver
            .execute_http_request(
                &request.url,
                request.method.as_str(),
                headers,
                Some(request.effective_body()),
                storage_path,
            )
            .await
            .map_err(|err| HarnessError::transport(err.client_message()))?;

        Ok(NormalizedResponse::from_http_headers(
            result.status,
            result.headers,
            result.body,
            self.id(),
        ))
    }

    async fn execute_chat(&self, request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        let selectors = if request.chat_selectors.is_empty() {
            self.chat_selectors.clone()
        } else {
            request.chat_selectors.clone()
        };

        let input = selectors
            .get("input")
            .cloned()
            .ok_or_else(|| HarnessError::config("chat selector `input` is required"))?;
        let submit = selectors
            .get("submit")
            .cloned()
            .ok_or_else(|| HarnessError::config("chat selector `submit` is required"))?;
        let response = selectors
            .get("response")
            .cloned()
            .ok_or_else(|| HarnessError::config("chat selector `response` is required"))?;

        let storage_path = request
            .auth
            .storage_state_path
            .as_deref()
            .map(Path::new)
            .or(self.storage_state_path.as_deref());

        let chat_response = self
            .driver
            .send_chat_prompt_ex(ChatPromptArgs {
                url: &request.url,
                prompt: &request.payload,
                input_selector: &input,
                submit_selector: &submit,
                response_selector: &response,
                storage_state_path: storage_path,
                file_input_selector: selectors.get("file").map(String::as_str),
                file_path: request.file_path.as_deref(),
                keep_page: request.keep_page,
                wait_stable_ms: request.wait_stable_ms,
                timeout_ms: request.timeout_ms,
            })
            .await
            .map_err(|err| HarnessError::transport(err.client_message()))?;

        Ok(NormalizedResponse::from_chat(chat_response, self.id()))
    }
}
