use std::sync::Arc;

use promptlab_core::{PromptLabError, PromptLabResult};
use tracing::instrument;

use crate::config::AuthEngineConfig;
use crate::cookies::{CookieManager, TokenExtractor};
use crate::mock::SharedPlaywrightDriver;
use crate::playwright::{parse_cookies, parse_tokens, PlaywrightClient};
use crate::secrets::migrate::resolve_auth_config_secrets;
use crate::session::SessionStore;
use crate::types::{
    AuthConfig, AuthMethod, AuthProfile, AuthSession, AuthenticateResult, ExtractedToken,
    LoginRecording, PlaywrightStorageState, RecordLoginOptions, RecordedStep, ReplayOptions,
    ReplayResult,
};

/// Authentication engine — login recording, session replay, token/cookie management.
pub struct AuthEngine {
    config: AuthEngineConfig,
    store: SessionStore,
    driver: SharedPlaywrightDriver,
}

impl AuthEngine {
    pub async fn new(
        config: AuthEngineConfig,
        store: SessionStore,
        driver: Option<SharedPlaywrightDriver>,
    ) -> PromptLabResult<Self> {
        let driver = match driver {
            Some(d) => d,
            None => Arc::new(PlaywrightClient::new(config.clone()).await?),
        };
        Ok(Self {
            config,
            store,
            driver,
        })
    }

    pub fn config(&self) -> &AuthEngineConfig {
        &self.config
    }

    pub fn store(&self) -> &SessionStore {
        &self.store
    }

    /// Record an interactive or automated browser login flow.
    #[instrument(skip(self, profile))]
    pub async fn record_login(
        &self,
        profile: &AuthProfile,
        options: RecordLoginOptions,
    ) -> PromptLabResult<(AuthSession, LoginRecording)> {
        let mut profile = profile.clone();
        resolve_auth_config_secrets(&mut profile.config, self.store.secrets())?;

        if !profile.method.uses_browser() {
            return Err(PromptLabError::invalid_input(format!(
                "method {} does not support browser recording",
                profile.method.as_str()
            )));
        }

        let login_url = profile
            .config
            .login_url()
            .ok_or_else(|| PromptLabError::invalid_input("profile missing login_url"))?;

        let pw_config = config_to_playwright(&profile.config)?;
        let result = self
            .driver
            .record_login(
                login_url,
                profile.method.as_str(),
                pw_config,
                options,
            )
            .await?;

        self.persist_record_result(&profile.id, result).await
    }

    /// Launch a headed browser for manual login at `login_url`.
    pub async fn begin_interactive_recording(
        &self,
        login_url: &str,
        options: RecordLoginOptions,
    ) -> PromptLabResult<()> {
        self.driver.begin_interactive_login(login_url, options).await
    }

    /// Capture browser state after the user finishes manual login.
    pub async fn finish_interactive_recording(
        &self,
        profile_id: &str,
    ) -> PromptLabResult<(AuthSession, LoginRecording)> {
        let result = self.driver.finish_interactive_login().await?;
        self.persist_record_result(profile_id, result).await
    }

    async fn persist_record_result(
        &self,
        profile_id: &str,
        result: crate::playwright::RecordLoginResult,
    ) -> PromptLabResult<(AuthSession, LoginRecording)> {
        let cookies = parse_cookies(&serde_json::Value::Array(result.cookies))?;
        let tokens = parse_tokens(&serde_json::Value::Array(result.tokens))?;
        let steps: Vec<RecordedStep> = result
            .steps
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();

        let storage: PlaywrightStorageState =
            serde_json::from_value(result.storage_state.clone())
                .unwrap_or(PlaywrightStorageState {
                    cookies: cookies.clone(),
                    origins: vec![],
                });

        let mut session = self
            .store
            .persist_session(profile_id, &cookies, &tokens, None)
            .await?;

        let storage_path = self
            .store
            .save_storage_state(&session.id, &storage)
            .await?;

        self.store
            .update_storage_path(&session.id, &storage_path)
            .await?;

        session.storage_state_path = Some(storage_path.to_string_lossy().into_owned());

        let recording = self
            .store
            .save_recording(
                profile_id,
                &steps,
                Some(storage_path.clone()),
                serde_json::json!({"final_url": result.final_url}),
            )
            .await?;

        Ok((session, recording))
    }

    /// Replay a stored session in the browser.
    #[instrument(skip(self))]
    pub async fn replay_session(
        &self,
        session_id: &str,
        url: &str,
        options: ReplayOptions,
    ) -> PromptLabResult<ReplayResult> {
        let session = self.store.get_session(session_id).await?;
        let storage_state = if let Some(path) = &session.storage_state_path {
            Some(
                serde_json::to_value(self.store.load_storage_state(std::path::Path::new(path)).await?)
                    .unwrap(),
            )
        } else {
            None
        };

        let result = self
            .driver
            .replay_session(
                url,
                storage_state,
                session.storage_state_path.as_deref().map(std::path::Path::new),
                options,
            )
            .await?;

        let cookies = parse_cookies(&serde_json::Value::Array(result.cookies))?;
        let tokens = parse_tokens(&serde_json::Value::Array(result.tokens))?;

        self.store
            .update_session_cookies(session_id, &cookies)
            .await?;
        self.store
            .update_session_tokens(session_id, &tokens)
            .await?;

        Ok(ReplayResult {
            session_id: session_id.to_string(),
            final_url: result.url,
            cookies,
            tokens,
        })
    }

    /// Authenticate using the configured method (browser or credential-based).
    #[instrument(skip(self, profile))]
    pub async fn authenticate(
        &self,
        profile: &AuthProfile,
        record_options: RecordLoginOptions,
    ) -> PromptLabResult<AuthenticateResult> {
        match profile.method {
            AuthMethod::Jwt => authenticate_jwt(profile, &self.store).await,
            AuthMethod::ApiKey => authenticate_api_key(profile, &self.store).await,
            _ => {
                let (session, recording) =
                    self.record_login(profile, record_options).await?;
                Ok(AuthenticateResult {
                    session,
                    recording: Some(recording),
                })
            }
        }
    }

    /// Extract tokens from an active session (refreshes from browser if needed).
    pub async fn extract_tokens(&self, session_id: &str, url: Option<&str>) -> PromptLabResult<Vec<ExtractedToken>> {
        let session = self.store.get_session(session_id).await?;
        let browser_tokens = self.driver.extract_tokens(url).await?;
        let merged = TokenExtractor::merge_tokens(&session.tokens, &browser_tokens);
        self.store
            .update_session_tokens(session_id, &merged)
            .await?;
        Ok(merged)
    }

    /// Export cookies from session storage.
    pub async fn export_cookies(&self, session_id: &str) -> PromptLabResult<Vec<crate::types::CookieRecord>> {
        CookieManager::new(&self.store, self.driver.as_ref())
            .export_cookies(session_id)
            .await
    }

    /// Import cookies into session and browser context.
    pub async fn import_cookies(
        &self,
        session_id: &str,
        cookies: Vec<crate::types::CookieRecord>,
    ) -> PromptLabResult<Vec<crate::types::CookieRecord>> {
        CookieManager::new(&self.store, self.driver.as_ref())
            .import_cookies(session_id, cookies)
            .await
    }

    pub async fn close(&self) -> PromptLabResult<()> {
        self.driver.close().await
    }
}

async fn authenticate_jwt(profile: &AuthProfile, store: &SessionStore) -> PromptLabResult<AuthenticateResult> {
    let mut profile = profile.clone();
    resolve_auth_config_secrets(&mut profile.config, store.secrets())?;
    let AuthConfig::Jwt { token, header_name, prefix, .. } = &profile.config else {
        return Err(PromptLabError::invalid_input("expected jwt config"));
    };
    if token.is_empty() {
        return Err(PromptLabError::invalid_input("jwt token is missing"));
    }
    TokenExtractor::validate_jwt_structure(token)?;
    let extracted =
        TokenExtractor::from_jwt_config(token, header_name.as_deref(), prefix.as_deref());
    let session = store
        .persist_session(&profile.id, &[], &[extracted], None)
        .await?;
    Ok(AuthenticateResult {
        session,
        recording: None,
    })
}

async fn authenticate_api_key(profile: &AuthProfile, store: &SessionStore) -> PromptLabResult<AuthenticateResult> {
    let mut profile = profile.clone();
    resolve_auth_config_secrets(&mut profile.config, store.secrets())?;
    let AuthConfig::ApiKey { key, header_name, prefix, .. } = &profile.config else {
        return Err(PromptLabError::invalid_input("expected api_key config"));
    };
    if key.is_empty() {
        return Err(PromptLabError::invalid_input("api key must not be empty"));
    }
    let token = ExtractedToken {
        kind: "api_key".into(),
        source: "config".into(),
        value: if let Some(p) = prefix {
            format!("{p}{key}")
        } else {
            key.clone()
        },
        url: None,
        // Persist the target header so downstream HTTP clients can apply the
        // credential as `{header_name}: {value}`.
        header_name: Some(header_name.clone()),
    };
    let session = store
        .persist_session(&profile.id, &[], &[token], None)
        .await?;
    Ok(AuthenticateResult {
        session,
        recording: None,
    })
}

fn config_to_playwright(config: &AuthConfig) -> PromptLabResult<serde_json::Value> {
    match config {
        AuthConfig::UsernamePassword {
            username,
            password,
            username_selector,
            password_selector,
            submit_selector,
            ..
        } => Ok(serde_json::json!({
            "username": username,
            "password": password,
            "username_selector": username_selector,
            "password_selector": password_selector,
            "submit_selector": submit_selector,
        })),
        AuthConfig::OAuth {
            success_url_pattern, ..
        } => Ok(serde_json::json!({
            "success_url_pattern": success_url_pattern,
        })),
        AuthConfig::Oidc {
            success_url_pattern,
            issuer,
            client_id,
            ..
        } => Ok(serde_json::json!({
            "success_url_pattern": success_url_pattern,
            "issuer": issuer,
            "client_id": client_id,
        })),
        AuthConfig::Saml {
            success_url_pattern,
            idp_entity_id,
            ..
        } => Ok(serde_json::json!({
            "success_url_pattern": success_url_pattern,
            "idp_entity_id": idp_entity_id,
        })),
        AuthConfig::Jwt { .. } | AuthConfig::ApiKey { .. } => Err(PromptLabError::invalid_input(
            "credential auth does not use playwright config",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockPlaywrightDriver;
    use promptlab_storage::Database;
    use std::sync::Arc;

    // Returns the engine together with the vault TempDir, which must be kept
    // alive for the duration of the test (dropping it deletes the vault).
    async fn test_engine() -> (AuthEngine, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let store = SessionStore::new(db, dir.path()).await.unwrap();
        let driver = Arc::new(MockPlaywrightDriver::login_success());
        let engine =
            AuthEngine::new(AuthEngineConfig::default().with_vault_dir(dir.path()), store, Some(driver))
                .await
                .unwrap();
        (engine, dir)
    }

    #[tokio::test]
    async fn records_login_and_replays_session() {
        let (engine, _vault) = test_engine().await;
        let profile = AuthProfile {
            id: String::new(),
            project_id: None,
            name: "Test".into(),
            method: AuthMethod::UsernamePassword,
            config: AuthConfig::UsernamePassword {
                login_url: "https://example.com/login".into(),
                username: Some("user".into()),
                password: Some("pass".into()),
                password_credential_id: None,
                username_selector: "#user".into(),
                password_selector: "#pass".into(),
                submit_selector: "#submit".into(),
            },
        };
        // Sessions reference a real profile (FK), so persist it first.
        let profile = engine.store().create_profile(&profile).await.unwrap();

        let (session, recording) = engine
            .record_login(&profile, RecordLoginOptions::default())
            .await
            .unwrap();

        assert!(!session.cookies.is_empty());
        assert!(!recording.steps.is_empty());
        assert!(session.storage_state_path.is_some());

        let replay = engine
            .replay_session(&session.id, "https://example.com/app", ReplayOptions::default())
            .await
            .unwrap();

        assert!(replay.final_url.contains("dashboard"));
    }

    #[tokio::test]
    async fn jwt_and_api_key_auth_without_browser() {
        let (engine, _vault) = test_engine().await;

        let jwt_profile = AuthProfile {
            id: String::new(),
            project_id: None,
            name: "JWT".into(),
            method: AuthMethod::Jwt,
            config: AuthConfig::Jwt {
                token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyIn0.sig".into(),
                token_credential_id: None,
                header_name: Some("Authorization".into()),
                prefix: Some("Bearer".into()),
            },
        };
        let jwt_profile = engine.store().create_profile(&jwt_profile).await.unwrap();

        let jwt_result = engine
            .authenticate(&jwt_profile, RecordLoginOptions::default())
            .await
            .unwrap();
        assert_eq!(jwt_result.session.tokens[0].kind, "jwt");
        // header_name + scheme prefix are persisted so the token is ready to send.
        assert_eq!(
            jwt_result.session.tokens[0].header_name.as_deref(),
            Some("Authorization")
        );
        assert!(jwt_result.session.tokens[0].value.starts_with("Bearer "));

        let api_profile = AuthProfile {
            id: String::new(),
            project_id: None,
            name: "API".into(),
            method: AuthMethod::ApiKey,
            config: AuthConfig::ApiKey {
                key: "secret-key".into(),
                key_credential_id: None,
                header_name: "X-API-Key".into(),
                prefix: None,
            },
        };
        let api_profile = engine.store().create_profile(&api_profile).await.unwrap();

        let api_result = engine
            .authenticate(&api_profile, RecordLoginOptions::default())
            .await
            .unwrap();
        assert_eq!(api_result.session.tokens[0].kind, "api_key");
        // The target header is retained (previously discarded via `let _ =`).
        assert_eq!(
            api_result.session.tokens[0].header_name.as_deref(),
            Some("X-API-Key")
        );
    }
}
