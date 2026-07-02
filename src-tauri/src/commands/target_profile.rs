//! Target Profile IPC — templates, verification, persistence.

use aisec_auth::{resolve_descriptor_for_wizard, SecretStore};
use aisec_core::{AisecError, LogCategory};
use aisec_storage::TargetRepository;
use aisec_target_profile::{
    build_wizard_attack_plan_with_llm, execute_verify_http, list_provider_templates,
    validate_http_response_with_llm, TargetProfile,
    VerifyHttpSuccess,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, warn};

use crate::dto::{TargetDto, TargetProfileDto, VerificationConsoleEntryDto};
use crate::error::{CommandError, CommandResult};
use crate::inference_host::{is_inference_ready, HostEndpointVerifyLlm, HostWizardPlannerLlm};
use crate::state::AppState;

fn profile_from_json(raw: &str) -> CommandResult<TargetProfile> {
    if raw.trim().is_empty() || raw == "{}" {
        return Ok(TargetProfile::default());
    }
    serde_json::from_str(raw).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })
}

fn profile_to_json(profile: &TargetProfile) -> CommandResult<String> {
    serde_json::to_string(profile).map_err(|err| {
        CommandError::from(AisecError::internal(err.to_string()))
    })
}

fn normalize_credential_prefix(prefix: &str) -> String {
    let scheme = prefix.trim();
    if scheme.is_empty() {
        return String::new();
    }
    if scheme.eq_ignore_ascii_case("basic")
        || scheme.eq_ignore_ascii_case("bearer")
        || scheme.eq_ignore_ascii_case("token")
    {
        return format!("{scheme} ");
    }
    prefix.to_string()
}

fn format_auth_header_value(prefix: Option<&str>, secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    match prefix {
        Some(p) if !p.is_empty() => {
            let scheme = p.trim();
            if trimmed.starts_with(p)
                || (!scheme.is_empty()
                    && trimmed
                        .to_ascii_lowercase()
                        .starts_with(&format!("{} ", scheme.to_ascii_lowercase())))
            {
                trimmed.to_string()
            } else {
                format!("{}{trimmed}", normalize_credential_prefix(p))
            }
        }
        _ => trimmed.to_string(),
    }
}

#[cfg(test)]
mod auth_header_tests {
    use super::format_auth_header_value;

    #[test]
    fn basic_prefix_without_space_inserts_separator() {
        assert_eq!(
            format_auth_header_value(Some("Basic"), "eXlwYXQ="),
            "Basic eXlwYXQ="
        );
    }

    #[test]
    fn basic_prefix_with_space_is_unchanged() {
        assert_eq!(
            format_auth_header_value(Some("Basic "), "eXlwYXQ="),
            "Basic eXlwYXQ="
        );
    }
}

fn insert_auth_header(
    headers: &mut std::collections::HashMap<String, String>,
    header_name: &str,
    prefix: Option<&str>,
    secret: &str,
) {
    let value = format_auth_header_value(prefix, secret);
    if value.is_empty() {
        return;
    }
    headers.insert(header_name.to_string(), value);
}

fn auth_headers_from_auth_value(
    auth: &serde_json::Value,
) -> std::collections::HashMap<String, String> {
    let mut headers = std::collections::HashMap::new();

    if let Some(config) = auth.get("config").and_then(|v| v.as_object()) {
        let kind = auth
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if kind == "api_key" {
            if let Some(key) = config.get("key").and_then(|v| v.as_str()) {
                let header_name = config
                    .get("header_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Authorization");
                let prefix = config.get("prefix").and_then(|v| v.as_str());
                insert_auth_header(&mut headers, header_name, prefix, key);
            }
        }

        if kind == "jwt" {
            if let Some(token) = config.get("token").and_then(|v| v.as_str()) {
                let header_name = config
                    .get("header_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Authorization");
                let prefix = config.get("prefix").and_then(|v| v.as_str()).or(Some("Bearer "));
                insert_auth_header(&mut headers, header_name, prefix, token);
            }
        }

        if kind == "basic" {
            if let (Some(user), Some(pass)) = (
                config.get("username").and_then(|v| v.as_str()),
                config.get("password").and_then(|v| v.as_str()),
            ) {
                use base64::Engine;
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
                headers.insert("Authorization".into(), format!("Basic {encoded}"));
            }
        }
    }

    // Legacy top-level auth fields.
    if let Some(token) = auth
        .get("token")
        .or_else(|| auth.get("api_key"))
        .and_then(|v| v.as_str())
    {
        let header_name = auth
            .get("header_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Authorization");
        let prefix = auth.get("prefix").and_then(|v| v.as_str());
        insert_auth_header(&mut headers, header_name, prefix, token);
    }

    if let Some(map) = auth.get("headers").and_then(|v| v.as_object()) {
        for (key, val) in map {
            if let Some(text) = val.as_str() {
                headers.insert(key.clone(), text.to_string());
            }
        }
    }

    if let (Some(user), Some(pass)) = (
        auth.get("username").and_then(|v| v.as_str()),
        auth.get("password").and_then(|v| v.as_str()),
    ) {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        headers.insert("Authorization".into(), format!("Basic {encoded}"));
    }

    headers
}

fn merge_auth_headers(
    base: &mut std::collections::HashMap<String, String>,
    overlay: std::collections::HashMap<String, String>,
) {
    for (key, value) in overlay {
        base.insert(key, value);
    }
}

fn auth_headers_from_descriptor(
    descriptor_json: &str,
) -> CommandResult<std::collections::HashMap<String, String>> {
    let secrets = SecretStore::new().map_err(CommandError::from)?;
    let resolved = resolve_descriptor_for_wizard(descriptor_json, &secrets)
        .map_err(CommandError::from)?;
    let value: serde_json::Value = serde_json::from_str(&resolved).map_err(|err| {
        CommandError::invalid_input(err.to_string())
    })?;

    let Some(auth) = value.get("auth") else {
        return Ok(std::collections::HashMap::new());
    };

    Ok(auth_headers_from_auth_value(auth))
}

fn resolve_verify_auth_headers(
    descriptor_json: &str,
    auth: Option<&serde_json::Value>,
    inline_auth_headers: Option<&std::collections::HashMap<String, String>>,
) -> CommandResult<std::collections::HashMap<String, String>> {
    if let Some(headers) = inline_auth_headers {
        if !headers.is_empty() {
            return Ok(headers.clone());
        }
    }

    let mut headers = auth_headers_from_descriptor(descriptor_json)?;
    if let Some(auth) = auth {
        merge_auth_headers(&mut headers, auth_headers_from_auth_value(auth));
    }
    Ok(headers)
}

pub async fn target_profile_list_templates_op() -> CommandResult<Vec<TargetProfileDto>> {
    Ok(list_provider_templates()
        .into_iter()
        .map(TargetProfileDto::from)
        .collect())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileSaveRequest {
    pub target_id: String,
    pub profile: serde_json::Value,
}

pub async fn target_profile_save_op(
    state: &AppState,
    request: TargetProfileSaveRequest,
) -> CommandResult<TargetDto> {
    let profile: TargetProfile = serde_json::from_value(request.profile).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })?;
    let json = profile_to_json(&profile)?;
    let target = state
        .repositories()
        .targets()
        .update_profile(&request.target_id, &json)
        .await
        .map_err(CommandError::from)?;
    Ok(TargetDto::from(target))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileVerifyRequest {
    pub target_id: String,
    pub profile: serde_json::Value,
    /// Inline auth from the wizard form — takes precedence over persisted descriptor secrets.
    pub auth: Option<serde_json::Value>,
    /// Resolved auth headers from the wizard form (highest priority).
    pub auth_headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileVerifyResponse {
    pub verified: bool,
    pub profile: TargetProfileDto,
    pub console: VerificationConsoleEntryDto,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileConnectVerifyResponse {
    pub success: bool,
    pub console: VerificationConsoleEntryDto,
    pub message: String,
    pub connect_snapshot: Option<VerifyHttpSuccess>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileVerifyAiRequest {
    pub target_id: String,
    pub profile: serde_json::Value,
    pub connect_snapshot: VerifyHttpSuccess,
}

pub async fn target_profile_verify_connect_op(
    state: &AppState,
    request: TargetProfileVerifyRequest,
) -> CommandResult<TargetProfileConnectVerifyResponse> {
    let TargetProfileVerifyRequest {
        target_id,
        profile: profile_value,
        auth,
        auth_headers: inline_auth_headers,
    } = request;

    let target = state
        .repositories()
        .targets()
        .get(&target_id)
        .await
        .map_err(CommandError::from)?;

    let auth_headers = resolve_verify_auth_headers(
        &target.descriptor_json,
        auth.as_ref(),
        inline_auth_headers.as_ref(),
    )?;

    let profile: TargetProfile = serde_json::from_value(profile_value).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })?;

    match execute_verify_http(&profile, auth_headers).await {
        Ok(snapshot) => Ok(TargetProfileConnectVerifyResponse {
            success: true,
            message: snapshot.console.message.clone(),
            console: VerificationConsoleEntryDto::from(snapshot.console.clone()),
            connect_snapshot: Some(snapshot),
        }),
        Err(attempt) => Ok(TargetProfileConnectVerifyResponse {
            success: false,
            message: attempt.console.message.clone(),
            console: VerificationConsoleEntryDto::from(attempt.console),
            connect_snapshot: None,
        }),
    }
}

pub async fn target_profile_verify_ai_op(
    state: &AppState,
    request: TargetProfileVerifyAiRequest,
) -> CommandResult<TargetProfileVerifyResponse> {
    let TargetProfileVerifyAiRequest {
        target_id,
        profile: profile_value,
        connect_snapshot,
    } = request;

    let mut profile: TargetProfile = serde_json::from_value(profile_value).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })?;

    let inference = state.inference_manager().lock().await;
    if !is_inference_ready(&inference) {
        return Err(CommandError::invalid_input(
            "AI Runtime must be ready before verifying the endpoint. Configure and load a model in AI Runtime.",
        ));
    }
    drop(inference);

    let llm_host = HostEndpointVerifyLlm::new(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );

    let attempt = validate_http_response_with_llm(&profile, &connect_snapshot, &llm_host).await;
    match attempt.result {
        Ok(verification) => {
            profile.verification = verification;
            let json = profile_to_json(&profile)?;
            state
                .repositories()
                .targets()
                .update_profile(&target_id, &json)
                .await
                .map_err(CommandError::from)?;

            Ok(TargetProfileVerifyResponse {
                verified: true,
                profile: TargetProfileDto::from(profile),
                console: VerificationConsoleEntryDto::from(attempt.console.clone()),
                message: attempt.console.message,
            })
        }
        Err(err) => {
            let message = err.to_string();
            profile.verification = aisec_target_profile::VerificationResult {
                verified: false,
                verified_at: None,
                provider: profile.provider.as_str().into(),
                model: None,
                capabilities: profile.default_capabilities.clone(),
                response_time_ms: attempt.console.response_time_ms,
                status_code: attempt.console.status_code,
                status: "failed".into(),
                response_preview: attempt.console.response_preview.clone(),
                error_message: Some(message.clone()),
            };
            let json = profile_to_json(&profile)?;
            state
                .repositories()
                .targets()
                .update_profile(&target_id, &json)
                .await
                .map_err(CommandError::from)?;

            Ok(TargetProfileVerifyResponse {
                verified: false,
                profile: TargetProfileDto::from(profile),
                console: VerificationConsoleEntryDto::from(attempt.console),
                message,
            })
        }
    }
}

pub async fn target_profile_verify_op(
    state: &AppState,
    request: TargetProfileVerifyRequest,
) -> CommandResult<TargetProfileVerifyResponse> {
    let connect = target_profile_verify_connect_op(state, request.clone()).await?;
    if !connect.success {
        let profile: TargetProfile = serde_json::from_value(request.profile).map_err(|err| {
            CommandError::invalid_input(format!("invalid target profile: {err}"))
        })?;
        return Ok(TargetProfileVerifyResponse {
            verified: false,
            profile: TargetProfileDto::from(profile),
            console: connect.console,
            message: connect.message,
        });
    }

    let snapshot = connect.connect_snapshot.ok_or_else(|| {
        CommandError::invalid_input("connect verification succeeded without snapshot")
    })?;

    target_profile_verify_ai_op(
        state,
        TargetProfileVerifyAiRequest {
            target_id: request.target_id,
            profile: request.profile,
            connect_snapshot: snapshot,
        },
    )
    .await
}

pub async fn target_profile_get_op(
    state: &AppState,
    target_id: String,
) -> CommandResult<TargetProfileDto> {
    let target = state
        .repositories()
        .targets()
        .get(&target_id)
        .await
        .map_err(CommandError::from)?;
    let profile = profile_from_json(&target.profile_json)?;
    Ok(TargetProfileDto::from(profile))
}

pub async fn planner_generate_from_profile_op(
    state: &AppState,
    target_id: String,
) -> CommandResult<crate::commands::planner::WizardAttackPlanDto> {
    let target = state
        .repositories()
        .targets()
        .get(&target_id)
        .await
        .map_err(CommandError::from)?;
    let profile = profile_from_json(&target.profile_json)?;
    if !profile.is_verified() {
        return Err(CommandError::invalid_input(
            "Target profile must be verified before attack planning",
        ));
    }

    let inference = state.inference_manager().lock().await;
    if !is_inference_ready(&inference) {
        return Err(CommandError::invalid_input(
            "AI Runtime must be ready before generating an attack plan. Configure and load a model in AI Runtime settings.",
        ));
    }
    drop(inference);

    let llm_host = HostWizardPlannerLlm::new(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );

    info!(
        target_id = %target_id,
        endpoint = %profile.full_url(),
        "wizard attack plan: starting AI Runtime planning"
    );

    let plan = match build_wizard_attack_plan_with_llm(&profile, &llm_host).await {
        Ok(plan) => plan,
        Err(err) => {
            let message = err.to_string();
            warn!(
                target_id = %target_id,
                endpoint = %profile.full_url(),
                error = %message,
                "wizard attack plan failed"
            );
            state.event_bus().error(
                LogCategory::Planner,
                "wizard_plan_generate",
                "promptlab-desktop",
                "target_profile",
                &message,
            );
            return Err(CommandError::invalid_input(message));
        }
    };

    state.event_bus().info(
        LogCategory::Planner,
        "wizard_plan_generate",
        "promptlab-desktop",
        "target_profile",
        format!(
            "AI attack plan generated for {} ({} modes)",
            profile.full_url(),
            plan.profile_modes.len()
        ),
    );

    Ok(crate::commands::planner::wizard_plan_to_dto(plan))
}

#[tauri::command]
pub async fn target_profile_list_templates() -> CommandResult<Vec<TargetProfileDto>> {
    target_profile_list_templates_op().await
}

#[tauri::command]
pub async fn target_profile_save(
    state: State<'_, AppState>,
    target_id: String,
    profile: serde_json::Value,
) -> CommandResult<TargetDto> {
    target_profile_save_op(
        state.inner(),
        TargetProfileSaveRequest {
            target_id,
            profile,
        },
    )
    .await
}

#[tauri::command]
pub async fn target_profile_verify_connect(
    state: State<'_, AppState>,
    target_id: String,
    profile: serde_json::Value,
    auth: Option<serde_json::Value>,
    auth_headers: Option<std::collections::HashMap<String, String>>,
) -> CommandResult<TargetProfileConnectVerifyResponse> {
    target_profile_verify_connect_op(
        state.inner(),
        TargetProfileVerifyRequest {
            target_id,
            profile,
            auth,
            auth_headers,
        },
    )
    .await
}

#[tauri::command]
pub async fn target_profile_verify_ai(
    state: State<'_, AppState>,
    target_id: String,
    profile: serde_json::Value,
    connect_snapshot: VerifyHttpSuccess,
) -> CommandResult<TargetProfileVerifyResponse> {
    target_profile_verify_ai_op(
        state.inner(),
        TargetProfileVerifyAiRequest {
            target_id,
            profile,
            connect_snapshot,
        },
    )
    .await
}

#[tauri::command]
pub async fn target_profile_verify(
    state: State<'_, AppState>,
    target_id: String,
    profile: serde_json::Value,
    auth: Option<serde_json::Value>,
    auth_headers: Option<std::collections::HashMap<String, String>>,
) -> CommandResult<TargetProfileVerifyResponse> {
    target_profile_verify_op(
        state.inner(),
        TargetProfileVerifyRequest {
            target_id,
            profile,
            auth,
            auth_headers,
        },
    )
    .await
}

#[tauri::command]
pub async fn target_profile_get(
    state: State<'_, AppState>,
    target_id: String,
) -> CommandResult<TargetProfileDto> {
    target_profile_get_op(state.inner(), target_id).await
}

#[tauri::command]
pub async fn planner_generate_from_profile(
    state: State<'_, AppState>,
    target_id: String,
) -> CommandResult<crate::commands::planner::WizardAttackPlanDto> {
    planner_generate_from_profile_op(state.inner(), target_id).await
}

pub fn parse_target_profile(raw: &str) -> CommandResult<TargetProfile> {
    profile_from_json(raw)
}

pub fn serialize_target_profile(profile: &TargetProfile) -> CommandResult<String> {
    profile_to_json(profile)
}
