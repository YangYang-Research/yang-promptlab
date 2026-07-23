//! Target Profile IPC — templates, verification, persistence.

use aisec_auth::{resolve_descriptor_for_wizard, SecretStore};
use aisec_core::{AisecError, LogCategory};
use aisec_storage::TargetRepository;
use aisec_target_profile::{
    execute_capability_probe, execute_verify_http, has_ai_response, list_provider_templates,
    TargetProfile, VerificationError, VerifyHttpSuccess,
};
use aisec_agent::{MemoryContext, YazgDelegation, YazgSupervisor};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, warn};

use crate::agent_memory::SqliteAgentMemoryStore;
use crate::dto::{TargetDto, TargetProfileDto, VerificationConsoleEntryDto};
use crate::error::{CommandError, CommandResult};
use crate::inference_host::{is_inference_ready, YazgHostLlms};
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

pub(crate) fn auth_headers_from_descriptor(
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
    /// Fresh Step 2 capability-probe HTTP console (before Yazg classification).
    pub probe_console: Option<VerificationConsoleEntryDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileConnectVerifyResponse {
    pub success: bool,
    pub console: VerificationConsoleEntryDto,
    pub message: String,
    pub connect_snapshot: Option<VerifyHttpSuccess>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileCapabilityVerifyResponse {
    pub success: bool,
    pub console: VerificationConsoleEntryDto,
    pub message: String,
    /// Fresh capability-probe snapshot for Yazg classification (when success).
    pub capability_snapshot: Option<VerifyHttpSuccess>,
    pub profile: TargetProfileDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileVerifyAiRequest {
    pub target_id: String,
    pub profile: serde_json::Value,
    /// Inline auth from the wizard form — used to re-send the Step 2 capability probe.
    pub auth: Option<serde_json::Value>,
    /// Resolved auth headers from the wizard form (highest priority).
    pub auth_headers: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileVerifyAiClassifyRequest {
    pub target_id: String,
    pub profile: serde_json::Value,
    pub capability_snapshot: VerifyHttpSuccess,
}

async fn persist_failed_verification(
    state: &AppState,
    target_id: &str,
    profile: &mut TargetProfile,
    console: &aisec_target_profile::VerificationConsoleEntry,
    message: &str,
) -> CommandResult<()> {
    profile.verification = aisec_target_profile::VerificationResult {
        verified: false,
        verified_at: None,
        provider: profile.provider.as_str().into(),
        model: None,
        capabilities: profile.default_capabilities.clone(),
        response_time_ms: console.response_time_ms,
        status_code: console.status_code,
        status: "failed".into(),
        response_preview: console.response_preview.clone(),
        error_message: Some(message.to_string()),
    };
    let json = profile_to_json(profile)?;
    state
        .repositories()
        .targets()
        .update_profile(target_id, &json)
        .await
        .map_err(CommandError::from)?;
    Ok(())
}

/// Step 2a — capability probe only (HTTP). Frontend can render this before Yazg runs.
pub async fn target_profile_verify_capability_op(
    state: &AppState,
    request: TargetProfileVerifyAiRequest,
) -> CommandResult<TargetProfileCapabilityVerifyResponse> {
    let TargetProfileVerifyAiRequest {
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

    let mut profile: TargetProfile = serde_json::from_value(profile_value).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })?;

    let http = match execute_capability_probe(&profile, auth_headers).await {
        Ok(snapshot) => snapshot,
        Err(attempt) => {
            let message = attempt.console.message.clone();
            persist_failed_verification(
                state,
                &target_id,
                &mut profile,
                &attempt.console,
                &message,
            )
            .await?;
            let console = VerificationConsoleEntryDto::from(attempt.console);
            return Ok(TargetProfileCapabilityVerifyResponse {
                success: false,
                console,
                message,
                capability_snapshot: None,
                profile: TargetProfileDto::from(profile),
            });
        }
    };

    let probe_console = VerificationConsoleEntryDto::from(http.console.clone());
    let status = reqwest::StatusCode::from_u16(http.status_code).unwrap_or(reqwest::StatusCode::OK);
    if !has_ai_response(&http.response_text, status) {
        let message = if http.response_text.trim().is_empty() {
            "capability probe returned an empty body — try again or increase model latency budget"
                .into()
        } else {
            VerificationError::NoAiResponse.to_string()
        };
        persist_failed_verification(state, &target_id, &mut profile, &http.console, &message)
            .await?;
        return Ok(TargetProfileCapabilityVerifyResponse {
            success: false,
            console: VerificationConsoleEntryDto {
                success: false,
                message: message.clone(),
                ..probe_console
            },
            message,
            capability_snapshot: None,
            profile: TargetProfileDto::from(profile),
        });
    }

    Ok(TargetProfileCapabilityVerifyResponse {
        success: true,
        console: probe_console,
        message: http.console.message.clone(),
        capability_snapshot: Some(http),
        profile: TargetProfileDto::from(profile),
    })
}

/// Step 2b — Yazg / AnalyzeEndpointAgent classification of an already-captured probe.
pub async fn target_profile_verify_ai_classify_op(
    state: &AppState,
    request: TargetProfileVerifyAiClassifyRequest,
) -> CommandResult<TargetProfileVerifyResponse> {
    let TargetProfileVerifyAiClassifyRequest {
        target_id,
        profile: profile_value,
        capability_snapshot: http,
    } = request;

    let mut profile: TargetProfile = serde_json::from_value(profile_value).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })?;

    let inference = state.inference_manager().lock().await;
    if !is_inference_ready(&inference) {
        return Err(CommandError::invalid_input(
            "Yazg Agent is offline. Configure and start AI Runtime so Yazg is Live before verifying the endpoint.",
        ));
    }
    drop(inference);

    let probe_console = VerificationConsoleEntryDto::from(http.console.clone());

    let hosts = YazgHostLlms::from_app(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );
    let llms = hosts.react_llms();
    let memory = SqliteAgentMemoryStore::new(state.repositories());
    let memory_ctx = MemoryContext::new(format!("wizard-verify:{target_id}"))
        .with_target(Some(target_id.clone()));

    let delegation =
        YazgSupervisor::react_classify_probe(&profile, &http, &llms, Some(&memory), memory_ctx)
            .await;
    match delegation {
        Ok(YazgDelegation::AnalyzedEndpoint { outcome, .. }) => {
            profile.verification = outcome.verification;
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
                console: VerificationConsoleEntryDto::from(outcome.console.clone()),
                message: outcome.console.message,
                probe_console: Some(probe_console),
            })
        }
        Ok(other) => {
            let message = match &other {
                YazgDelegation::Chat { turn }
                | YazgDelegation::Planned { turn, .. }
                | YazgDelegation::GeneratedPrompt { turn, .. }
                | YazgDelegation::Recommended { turn, .. }
                | YazgDelegation::Summarized { turn, .. }
                | YazgDelegation::Judged { turn, .. }
                | YazgDelegation::ExecutedAttack { turn, .. }
                | YazgDelegation::CreatedProject { turn, .. }
                | YazgDelegation::AnalyzedEndpoint { turn, .. } => turn.reply.clone(),
            };
            let message = if message.trim().is_empty() {
                "Yazg ReAct finished without AnalyzeEndpointAgent confirmation".into()
            } else {
                message
            };
            persist_failed_verification(state, &target_id, &mut profile, &http.console, &message)
                .await?;

            Ok(TargetProfileVerifyResponse {
                verified: false,
                profile: TargetProfileDto::from(profile),
                console: VerificationConsoleEntryDto {
                    success: false,
                    message: message.clone(),
                    ..probe_console.clone()
                },
                message,
                probe_console: Some(probe_console),
            })
        }
        Err(err) => {
            let message = err.to_string();
            persist_failed_verification(state, &target_id, &mut profile, &http.console, &message)
                .await?;

            Ok(TargetProfileVerifyResponse {
                verified: false,
                profile: TargetProfileDto::from(profile),
                console: VerificationConsoleEntryDto {
                    success: false,
                    message: message.clone(),
                    ..probe_console.clone()
                },
                message,
                probe_console: Some(probe_console),
            })
        }
    }
}

/// Combined Step 2 — capability probe then Yazg classification (legacy / one-shot callers).
pub async fn target_profile_verify_ai_op(
    state: &AppState,
    request: TargetProfileVerifyAiRequest,
) -> CommandResult<TargetProfileVerifyResponse> {
    let capability = target_profile_verify_capability_op(state, request.clone()).await?;
    if !capability.success {
        return Ok(TargetProfileVerifyResponse {
            verified: false,
            profile: capability.profile,
            console: capability.console.clone(),
            message: capability.message,
            probe_console: Some(capability.console),
        });
    }

    let Some(snapshot) = capability.capability_snapshot else {
        return Ok(TargetProfileVerifyResponse {
            verified: false,
            profile: capability.profile,
            console: capability.console.clone(),
            message: capability.message,
            probe_console: Some(capability.console),
        });
    };

    target_profile_verify_ai_classify_op(
        state,
        TargetProfileVerifyAiClassifyRequest {
            target_id: request.target_id,
            profile: request.profile,
            capability_snapshot: snapshot,
        },
    )
    .await
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
            console: connect.console.clone(),
            message: connect.message,
            probe_console: Some(connect.console),
        });
    }

    target_profile_verify_ai_op(
        state,
        TargetProfileVerifyAiRequest {
            target_id: request.target_id,
            profile: request.profile,
            auth: request.auth,
            auth_headers: request.auth_headers,
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
            "Yazg Agent is offline. Configure and start AI Runtime so Yazg is Live before generating an attack plan.",
        ));
    }
    drop(inference);

    let hosts = YazgHostLlms::from_app(
        state.data_dir().to_path_buf(),
        state.inference_manager().clone(),
        state.model_manager().clone(),
        state.model_provider().clone(),
        state.runtime_manager().clone(),
    );
    let llms = hosts.react_llms();
    let memory = SqliteAgentMemoryStore::new(state.repositories());
    let memory_ctx = MemoryContext::new(format!("wizard-plan:{target_id}"))
        .with_project(Some(target.project_id.clone()))
        .with_target(Some(target_id.clone()));

    info!(
        target_id = %target_id,
        endpoint = %profile.full_url(),
        "wizard attack plan: Yazg supervisor (soft AttackPlan hint)"
    );

    let plan = match YazgSupervisor::react_plan(&profile, &llms, Some(&memory), memory_ctx).await {
        Ok(YazgDelegation::Planned { outcome, .. }) => outcome.plan,
        Ok(other) => {
            let message = match other {
                YazgDelegation::Chat { turn }
                | YazgDelegation::AnalyzedEndpoint { turn, .. }
                | YazgDelegation::GeneratedPrompt { turn, .. }
                | YazgDelegation::Recommended { turn, .. }
                | YazgDelegation::Summarized { turn, .. }
                | YazgDelegation::Judged { turn, .. }
                | YazgDelegation::ExecutedAttack { turn, .. }
                | YazgDelegation::CreatedProject { turn, .. }
                | YazgDelegation::Planned { turn, .. } => turn.reply,
            };
            warn!(
                target_id = %target_id,
                endpoint = %profile.full_url(),
                "wizard attack plan: ReAct finished without AttackPlanAgent"
            );
            return Err(CommandError::invalid_input(if message.trim().is_empty() {
                "Yazg ReAct did not produce an attack plan".into()
            } else {
                message
            }));
        }
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
    auth: Option<serde_json::Value>,
    auth_headers: Option<std::collections::HashMap<String, String>>,
) -> CommandResult<TargetProfileVerifyResponse> {
    target_profile_verify_ai_op(
        state.inner(),
        TargetProfileVerifyAiRequest {
            target_id,
            profile,
            auth,
            auth_headers,
        },
    )
    .await
}

#[tauri::command]
pub async fn target_profile_verify_capability(
    state: State<'_, AppState>,
    target_id: String,
    profile: serde_json::Value,
    auth: Option<serde_json::Value>,
    auth_headers: Option<std::collections::HashMap<String, String>>,
) -> CommandResult<TargetProfileCapabilityVerifyResponse> {
    target_profile_verify_capability_op(
        state.inner(),
        TargetProfileVerifyAiRequest {
            target_id,
            profile,
            auth,
            auth_headers,
        },
    )
    .await
}

#[tauri::command]
pub async fn target_profile_verify_ai_classify(
    state: State<'_, AppState>,
    target_id: String,
    profile: serde_json::Value,
    capability_snapshot: VerifyHttpSuccess,
) -> CommandResult<TargetProfileVerifyResponse> {
    target_profile_verify_ai_classify_op(
        state.inner(),
        TargetProfileVerifyAiClassifyRequest {
            target_id,
            profile,
            capability_snapshot,
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
