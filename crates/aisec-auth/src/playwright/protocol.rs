use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaywrightResponse {
    pub id: u64,
    pub ok: bool,
    #[serde(default)]
    pub result: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordLoginRequest {
    pub url: String,
    pub method: String,
    pub config: serde_json::Value,
    pub options: PlaywrightOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySessionRequest {
    pub url: String,
    #[serde(default)]
    pub storage_state: Option<serde_json::Value>,
    #[serde(default)]
    pub storage_state_path: Option<String>,
    pub options: PlaywrightOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaywrightOptions {
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub headed: bool,
    #[serde(default)]
    pub timeout_ms: u64,
    #[serde(default)]
    pub interactive_timeout_ms: u64,
    #[serde(default)]
    pub storage_state_path: Option<String>,
    #[serde(default)]
    pub slow_mo: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecordLoginResult {
    pub steps: Vec<serde_json::Value>,
    pub storage_state: serde_json::Value,
    pub cookies: Vec<serde_json::Value>,
    pub tokens: Vec<serde_json::Value>,
    pub final_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplaySessionResult {
    pub url: String,
    pub cookies: Vec<serde_json::Value>,
    pub tokens: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecuteHttpResult {
    pub status: u16,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
    pub duration_ms: u64,
}
