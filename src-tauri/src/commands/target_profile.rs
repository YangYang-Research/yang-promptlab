//! Target Profile IPC — templates, verification, persistence.

use aisec_auth::{resolve_descriptor_for_runtime, SecretStore};
use aisec_core::AisecError;
use aisec_storage::TargetRepository;
use aisec_target_profile::{
    list_provider_templates, plan_from_target_profile, verify_target_profile, TargetProfile,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::dto::{TargetDto, TargetProfileDto, VerificationConsoleEntryDto};
use crate::error::{CommandError, CommandResult};
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

fn auth_headers_from_descriptor(
    descriptor_json: &str,
) -> CommandResult<std::collections::HashMap<String, String>> {
    let secrets = SecretStore::new().map_err(CommandError::from)?;
    let resolved = resolve_descriptor_for_runtime(descriptor_json, &secrets)
        .map_err(CommandError::from)?;
    let value: serde_json::Value = serde_json::from_str(&resolved).map_err(|err| {
        CommandError::invalid_input(err.to_string())
    })?;

    let mut headers = std::collections::HashMap::new();
    if let Some(auth) = value.get("auth") {
        if let Some(token) = auth
            .get("token")
            .or_else(|| auth.get("api_key"))
            .and_then(|v| v.as_str())
        {
            let header_name = auth
                .get("header_name")
                .and_then(|v| v.as_str())
                .unwrap_or("Authorization");
            let prefix = auth.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
            headers.insert(header_name.to_string(), format!("{prefix}{token}"));
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
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
            headers.insert("Authorization".into(), format!("Basic {encoded}"));
        }
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetProfileVerifyResponse {
    pub verified: bool,
    pub profile: TargetProfileDto,
    pub console: VerificationConsoleEntryDto,
    pub message: String,
}

pub async fn target_profile_verify_op(
    state: &AppState,
    request: TargetProfileVerifyRequest,
) -> CommandResult<TargetProfileVerifyResponse> {
    let mut profile: TargetProfile = serde_json::from_value(request.profile).map_err(|err| {
        CommandError::invalid_input(format!("invalid target profile: {err}"))
    })?;

    let target = state
        .repositories()
        .targets()
        .get(&request.target_id)
        .await
        .map_err(CommandError::from)?;

    let auth_headers = auth_headers_from_descriptor(&target.descriptor_json)?;

    match verify_target_profile(&profile, auth_headers).await {
        Ok((verification, console)) => {
            profile.verification = verification;
            let json = profile_to_json(&profile)?;
            state
                .repositories()
                .targets()
                .update_profile(&request.target_id, &json)
                .await
                .map_err(CommandError::from)?;

            Ok(TargetProfileVerifyResponse {
                verified: true,
                profile: TargetProfileDto::from(profile),
                console: VerificationConsoleEntryDto::from(console),
                message: "Verification succeeded — target responded with AI content".into(),
            })
        }
        Err(err) => Err(CommandError::invalid_input(err.to_string())),
    }
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
    mode: String,
) -> CommandResult<crate::commands::planner::AttackPlanDto> {
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

    let plan = plan_from_target_profile(&profile);
    if mode.trim().eq_ignore_ascii_case("local_llm") {
        return Err(CommandError::invalid_input(
            "Local LLM planning from target profile is not yet supported — use deterministic mode",
        ));
    }
    Ok(crate::commands::planner::plan_to_dto(plan))
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
pub async fn target_profile_verify(
    state: State<'_, AppState>,
    target_id: String,
    profile: serde_json::Value,
) -> CommandResult<TargetProfileVerifyResponse> {
    target_profile_verify_op(
        state.inner(),
        TargetProfileVerifyRequest {
            target_id,
            profile,
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
    mode: String,
) -> CommandResult<crate::commands::planner::AttackPlanDto> {
    planner_generate_from_profile_op(state.inner(), target_id, mode).await
}

pub fn parse_target_profile(raw: &str) -> CommandResult<TargetProfile> {
    profile_from_json(raw)
}

pub fn serialize_target_profile(profile: &TargetProfile) -> CommandResult<String> {
    profile_to_json(profile)
}
