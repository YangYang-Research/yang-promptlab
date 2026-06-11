use serde::{Deserialize, Serialize};

/// Supported authentication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    UsernamePassword,
    OAuth,
    Oidc,
    Saml,
    Jwt,
    ApiKey,
}

impl AuthMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UsernamePassword => "username_password",
            Self::OAuth => "oauth",
            Self::Oidc => "oidc",
            Self::Saml => "saml",
            Self::Jwt => "jwt",
            Self::ApiKey => "api_key",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "username_password" => Some(Self::UsernamePassword),
            "oauth" => Some(Self::OAuth),
            "oidc" => Some(Self::Oidc),
            "saml" => Some(Self::Saml),
            "jwt" => Some(Self::Jwt),
            "api_key" => Some(Self::ApiKey),
            _ => None,
        }
    }

    pub fn uses_browser(self) -> bool {
        matches!(
            self,
            Self::UsernamePassword | Self::OAuth | Self::Oidc | Self::Saml
        )
    }
}

/// Profile configuration stored in `auth_profiles.config_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    UsernamePassword {
        login_url: String,
        username: Option<String>,
        password: Option<String>,
        username_selector: String,
        password_selector: String,
        submit_selector: String,
    },
    OAuth {
        login_url: String,
        success_url_pattern: Option<String>,
        provider: Option<String>,
    },
    Oidc {
        login_url: String,
        issuer: Option<String>,
        success_url_pattern: Option<String>,
        client_id: Option<String>,
    },
    Saml {
        login_url: String,
        success_url_pattern: Option<String>,
        idp_entity_id: Option<String>,
    },
    Jwt {
        token: String,
        header_name: Option<String>,
        prefix: Option<String>,
    },
    ApiKey {
        key: String,
        header_name: String,
        prefix: Option<String>,
    },
}

impl AuthConfig {
    pub fn login_url(&self) -> Option<&str> {
        match self {
            Self::UsernamePassword { login_url, .. }
            | Self::OAuth { login_url, .. }
            | Self::Oidc { login_url, .. }
            | Self::Saml { login_url, .. } => Some(login_url),
            Self::Jwt { .. } | Self::ApiKey { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfile {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub method: AuthMethod,
    pub config: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedStep {
    pub action: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedToken {
    pub kind: String,
    pub source: String,
    pub value: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieRecord {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default)]
    pub expires: Option<f64>,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub same_site: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaywrightStorageState {
    pub cookies: Vec<CookieRecord>,
    #[serde(default)]
    pub origins: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub id: String,
    pub profile_id: String,
    pub status: SessionStatus,
    pub cookies: Vec<CookieRecord>,
    pub tokens: Vec<ExtractedToken>,
    pub storage_state_path: Option<String>,
    pub expires_at: Option<time::OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Expired,
    Revoked,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRecording {
    pub id: String,
    pub profile_id: String,
    pub steps: Vec<RecordedStep>,
    pub storage_state_path: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordLoginOptions {
    pub headed: bool,
    pub timeout_ms: u64,
    pub interactive_timeout_ms: u64,
}

impl Default for RecordLoginOptions {
    fn default() -> Self {
        Self {
            headed: false,
            timeout_ms: 30_000,
            interactive_timeout_ms: 120_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayOptions {
    pub headed: bool,
    pub timeout_ms: u64,
}

impl Default for ReplayOptions {
    fn default() -> Self {
        Self {
            headed: false,
            timeout_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub session_id: String,
    pub final_url: String,
    pub cookies: Vec<CookieRecord>,
    pub tokens: Vec<ExtractedToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticateResult {
    pub session: AuthSession,
    pub recording: Option<LoginRecording>,
}
