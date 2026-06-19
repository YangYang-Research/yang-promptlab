use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// HTTP verb for harness delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    pub fn parse(method: &str) -> Option<Self> {
        match method.trim().to_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Post
    }
}

/// Authentication material attached to an attack request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthMaterial {
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub bearer_token: Option<String>,
    pub basic_username: Option<String>,
    pub basic_password: Option<String>,
    pub cookie_header: Option<String>,
    pub storage_state_path: Option<String>,
}

impl AuthMaterial {
    pub fn apply_to_headers(&self, headers: &mut HashMap<String, String>) {
        for (key, value) in &self.headers {
            headers.insert(key.clone(), value.clone());
        }
        if let Some(token) = &self.bearer_token {
            if !headers.keys().any(|k| k.eq_ignore_ascii_case("authorization")) {
                headers.insert("Authorization".into(), format!("Bearer {token}"));
            }
        }
        if let (Some(user), Some(pass)) = (&self.basic_username, &self.basic_password) {
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                format!("{user}:{pass}"),
            );
            headers.insert("Authorization".into(), format!("Basic {encoded}"));
        }
        if let Some(cookie) = &self.cookie_header {
            headers.insert("Cookie".into(), cookie.clone());
        }
    }
}

/// Unified attack delivery request consumed by every harness implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackRequest {
    pub url: String,
    #[serde(default)]
    pub method: HttpMethod,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub payload: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub auth: AuthMaterial,
    /// Optional chat UI selectors for Playwright harness.
    #[serde(default)]
    pub chat_selectors: HashMap<String, String>,
}

fn default_timeout_ms() -> u64 {
    30_000
}

impl AttackRequest {
    pub fn from_payload(url: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: HttpMethod::Post,
            headers: HashMap::new(),
            body: None,
            payload: payload.into(),
            timeout_ms: 30_000,
            auth: AuthMaterial::default(),
            chat_selectors: HashMap::new(),
        }
    }

    pub fn with_auth(mut self, auth: AuthMaterial) -> Self {
        self.auth = auth;
        self
    }

    pub fn effective_body(&self) -> String {
        if let Some(body) = &self.body {
            body.replace("{{payload}}", &self.payload)
        } else {
            self.payload.clone()
        }
    }

    pub fn merged_headers(&self) -> HashMap<String, String> {
        let mut headers = self.headers.clone();
        self.auth.apply_to_headers(&mut headers);
        if !headers.keys().any(|k| k.eq_ignore_ascii_case("content-type")) {
            headers.insert("Content-Type".into(), "application/json".into());
        }
        headers
    }
}
