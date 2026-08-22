use async_trait::async_trait;
use time::OffsetDateTime;

use crate::error::{HarnessError, HarnessResult};
use crate::models::{AttackRequest, HttpMethod, NormalizedResponse};
use crate::providers::HttpHarness;
use crate::sigv4::{sign_headers, SigV4Credentials};
use crate::traits::Harness;

/// AWS Bedrock `InvokeModel` with SigV4.
#[derive(Clone)]
pub struct BedrockHarness {
    inner: HttpHarness,
}

impl BedrockHarness {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            inner: HttpHarness::new()?,
        })
    }
}

impl Default for BedrockHarness {
    fn default() -> Self {
        Self::new().expect("bedrock harness")
    }
}

#[async_trait]
impl Harness for BedrockHarness {
    fn id(&self) -> &'static str {
        "bedrock"
    }

    async fn execute(&self, mut request: AttackRequest) -> HarnessResult<NormalizedResponse> {
        request.method = HttpMethod::Post;
        if request.body.is_none() {
            if request.has_chat_native() {
                request.body = Some(request.bedrock_converse_body());
            } else {
                request.body = Some(
                    serde_json::json!({
                        "anthropic_version": "bedrock-2023-05-31",
                        "max_tokens": 256,
                        "messages": [
                            { "role": "user", "content": "{{payload}}" }
                        ]
                    })
                    .to_string(),
                );
            }
        }
        let body = request.effective_body();
        let parsed = url::Url::parse(&request.url)
            .map_err(|err| HarnessError::config(format!("bedrock url: {err}")))?;
        let region = request
            .auth
            .aws_region
            .clone()
            .or_else(|| infer_region(parsed.host_str().unwrap_or("")))
            .unwrap_or_else(|| "us-east-1".into());
        let service = request
            .auth
            .aws_service
            .clone()
            .unwrap_or_else(|| "bedrock".into());
        let access = request.auth.aws_access_key_id.clone().ok_or_else(|| {
            HarnessError::auth("bedrock requires auth.awsAccessKeyId")
        })?;
        let secret = request.auth.aws_secret_access_key.clone().ok_or_else(|| {
            HarnessError::auth("bedrock requires auth.awsSecretAccessKey")
        })?;

        let mut headers = request.merged_headers();
        sign_headers(
            request.method.as_str(),
            &parsed,
            &mut headers,
            &body,
            SigV4Credentials {
                access_key_id: &access,
                secret_access_key: &secret,
                session_token: request.auth.aws_session_token.as_deref(),
                region: &region,
                service: &service,
            },
            OffsetDateTime::now_utc(),
        )?;
        request.headers = headers;
        request.auth.bearer_token = None;
        request.auth.api_key = None;
        request.body = Some(body);

        let mut response = self.inner.execute_raw(&request, self.id()).await?;
        response
            .metadata
            .insert("api_format".into(), "bedrock_invoke".into());
        Ok(response)
    }
}

fn infer_region(host: &str) -> Option<String> {
    // bedrock-runtime.us-east-1.amazonaws.com
    let rest = host.strip_prefix("bedrock-runtime.")?;
    let region = rest.split('.').next()?;
    if region.is_empty() {
        None
    } else {
        Some(region.to_string())
    }
}
