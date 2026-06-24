//! Persisted AI inference route (third-party cloud vs local llama runtime).

use std::path::{Path, PathBuf};

use aisec_models::{ModelEntry, ModelProvider};
use serde::{Deserialize, Serialize};

use crate::error::{CommandError, CommandResult};
use crate::third_party_credentials::{
    connectivity_status_label, format_connectivity_value, has_third_party_credentials_metadata,
    is_connectivity_success, short_connectivity_list_label, CONNECTIVITY_FAILED, CONNECTIVITY_SUCCESS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiInferenceRoute {
    ThirdParty,
    Local,
}

impl AiInferenceRoute {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ThirdParty => "third_party",
            Self::Local => "local",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "third_party" | "third-party" | "cloud" | "remote" => Some(Self::ThirdParty),
            "local" | "llama" | "embedded" => Some(Self::Local),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiInferenceSettings {
    pub route: AiInferenceRoute,
    pub selected_model_id: Option<String>,
    pub initialized: bool,
    #[serde(default)]
    pub third_party_connectivity: Option<String>,
    #[serde(default)]
    pub third_party_last_health_check: Option<String>,
}

impl Default for AiInferenceSettings {
    fn default() -> Self {
        Self {
            route: AiInferenceRoute::Local,
            selected_model_id: None,
            initialized: false,
            third_party_connectivity: None,
            third_party_last_health_check: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiInferenceModelOptionDto {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub verified: bool,
    pub configured: bool,
    pub status_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiInferenceSettingsDto {
    pub route: String,
    pub initialized: bool,
    pub selected_model_id: Option<String>,
    pub selected_model_name: Option<String>,
    pub third_party_available: bool,
    pub local_available: bool,
    pub third_party_models: Vec<AiInferenceModelOptionDto>,
    pub local_models: Vec<AiInferenceModelOptionDto>,
    pub message: String,
    /// Populated only when a connectivity test runs in the same request (not persisted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connectivity_test_ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connectivity_test_detail: Option<String>,
}

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ai_inference_settings.json")
}

pub async fn load_settings(data_dir: &Path) -> CommandResult<AiInferenceSettings> {
    let path = settings_path(data_dir);
    if !path.is_file() {
        return Ok(AiInferenceSettings::default());
    }
    let raw = tokio::fs::read_to_string(&path)
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    if raw.trim().is_empty() {
        tracing::warn!(
            path = %path.display(),
            "inference settings file is empty — using defaults"
        );
        return Ok(AiInferenceSettings::default());
    }
    match serde_json::from_str(&raw) {
        Ok(settings) => Ok(settings),
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "invalid inference settings JSON — using defaults"
            );
            Ok(AiInferenceSettings::default())
        }
    }
}

pub async fn save_settings(data_dir: &Path, settings: &AiInferenceSettings) -> CommandResult<()> {
    let path = settings_path(data_dir);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    }
    let raw = serde_json::to_string_pretty(settings)
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    let tmp_path = path.with_extension("json.tmp");
    tokio::fs::write(&tmp_path, raw)
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    tokio::fs::rename(&tmp_path, &path)
        .await
        .map_err(|err| CommandError::from(aisec_core::AisecError::internal(err.to_string())))?;
    Ok(())
}

pub fn is_third_party_model(entry: &ModelEntry) -> bool {
    entry.provider == ModelProvider::Remote
}

pub fn is_local_model(entry: &ModelEntry) -> bool {
    matches!(
        entry.provider,
        ModelProvider::Gguf | ModelProvider::HuggingFace | ModelProvider::Ollama
    )
}

pub fn is_configured_third_party(entry: &ModelEntry) -> bool {
    is_third_party_model(entry) && has_third_party_credentials_metadata(&entry.metadata)
}

pub fn is_configured_local(entry: &ModelEntry) -> bool {
    is_local_model(entry) && (entry.verified || entry.file_path.is_file())
}

fn local_model_option(entry: &ModelEntry, configured: bool) -> AiInferenceModelOptionDto {
    AiInferenceModelOptionDto {
        id: entry.id.clone(),
        name: entry.display_model_name(),
        provider: entry.display_provider(),
        verified: entry.verified,
        configured,
        status_label: if configured {
            "Ready".into()
        } else {
            "Needs setup".into()
        },
    }
}

fn third_party_model_option(
    entry: &ModelEntry,
    selected_model_id: Option<&str>,
    global_connectivity: Option<&str>,
) -> AiInferenceModelOptionDto {
    let configured = is_configured_third_party(entry);
    let status_label = if !configured {
        "Needs setup".into()
    } else if Some(entry.id.as_str()) == selected_model_id {
        global_connectivity
            .and_then(short_connectivity_list_label)
            .or_else(|| connectivity_status_label(&entry.metadata))
            .unwrap_or_else(|| "Not tested".into())
    } else {
        connectivity_status_label(&entry.metadata).unwrap_or_else(|| "Not tested".into())
    };

    AiInferenceModelOptionDto {
        id: entry.id.clone(),
        name: entry.display_model_name(),
        provider: entry.display_provider(),
        verified: entry.verified,
        configured,
        status_label,
    }
}

fn sort_local_models(mut entries: Vec<&ModelEntry>) -> Vec<AiInferenceModelOptionDto> {
    entries.sort_by(|a, b| {
        b.verified
            .cmp(&a.verified)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    entries
        .into_iter()
        .map(|entry| local_model_option(entry, is_configured_local(entry)))
        .collect()
}

fn sort_third_party_models(
    mut entries: Vec<&ModelEntry>,
    selected_model_id: Option<&str>,
    global_connectivity: Option<&str>,
) -> Vec<AiInferenceModelOptionDto> {
    entries.sort_by(|a, b| {
        b.verified
            .cmp(&a.verified)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    entries
        .into_iter()
        .map(|entry| third_party_model_option(entry, selected_model_id, global_connectivity))
        .collect()
}

fn pick_default_route(third_party: &[AiInferenceModelOptionDto], local: &[AiInferenceModelOptionDto]) -> AiInferenceRoute {
    let third_ready = third_party.iter().any(|m| m.configured);
    let local_ready = local.iter().any(|m| m.configured);
    if third_ready {
        AiInferenceRoute::ThirdParty
    } else if local_ready {
        AiInferenceRoute::Local
    } else if !third_party.is_empty() {
        AiInferenceRoute::ThirdParty
    } else {
        AiInferenceRoute::Local
    }
}

fn pick_model_for_route(
    route: AiInferenceRoute,
    third_party: &[AiInferenceModelOptionDto],
    local: &[AiInferenceModelOptionDto],
) -> Option<String> {
    let pool = match route {
        AiInferenceRoute::ThirdParty => third_party,
        AiInferenceRoute::Local => local,
    };
    pool.iter()
        .find(|m| m.configured)
        .or_else(|| pool.first())
        .map(|m| m.id.clone())
}

pub fn resolve_initial_settings(models: &[ModelEntry]) -> AiInferenceSettings {
    let third_party = sort_third_party_models(
        models.iter().filter(|e| is_third_party_model(e)).collect(),
        None,
        None,
    );
    let local = sort_local_models(models.iter().filter(|e| is_local_model(e)).collect());
    let route = pick_default_route(&third_party, &local);
    let selected_model_id = pick_model_for_route(route, &third_party, &local);
    AiInferenceSettings {
        route,
        selected_model_id,
        initialized: true,
        third_party_connectivity: None,
        third_party_last_health_check: None,
    }
}

pub fn reconcile_settings(
    mut settings: AiInferenceSettings,
    models: &[ModelEntry],
) -> AiInferenceSettings {
    let third_party = sort_third_party_models(
        models.iter().filter(|e| is_third_party_model(e)).collect(),
        settings.selected_model_id.as_deref(),
        settings.third_party_connectivity.as_deref(),
    );
    let local = sort_local_models(models.iter().filter(|e| is_local_model(e)).collect());

    let previous_selected = settings.selected_model_id.clone();

    if !settings.initialized {
        return resolve_initial_settings(models);
    }

    let pool = match settings.route {
        AiInferenceRoute::ThirdParty => &third_party,
        AiInferenceRoute::Local => &local,
    };

    let selected_valid = settings
        .selected_model_id
        .as_ref()
        .and_then(|id| pool.iter().find(|m| &m.id == id));

    if selected_valid.is_none() {
        settings.selected_model_id = pick_model_for_route(settings.route, &third_party, &local);
    }

    if settings.route == AiInferenceRoute::ThirdParty {
        let previous_third_party = previous_selected.as_deref().and_then(|id| {
            third_party
                .iter()
                .find(|model| model.id == id)
                .map(|model| model.id.as_str())
        });
        let selected_third_party = settings.selected_model_id.as_deref().and_then(|id| {
            third_party
                .iter()
                .find(|model| model.id == id)
                .map(|model| model.id.as_str())
        });
        if let (Some(previous), Some(selected)) = (previous_third_party, selected_third_party) {
            if previous != selected {
                settings.third_party_connectivity = None;
                settings.third_party_last_health_check = None;
            }
        }
    }

    settings
}

pub fn apply_third_party_health_check(
    settings: &mut AiInferenceSettings,
    checked_at: &str,
    ok: bool,
    latency_ms: u64,
) {
    settings.third_party_connectivity = Some(format_connectivity_value(ok, latency_ms));
    settings.third_party_last_health_check = Some(checked_at.to_string());
}

pub fn is_third_party_connectivity_ok(connectivity: Option<&str>) -> bool {
    connectivity.is_some_and(is_connectivity_success)
}

pub fn third_party_status_label(
    settings: &AiInferenceSettings,
    has_configured_model: bool,
    selected_model: Option<&ModelEntry>,
) -> String {
    if settings.selected_model_id.is_none() || !has_configured_model {
        return "N/A".into();
    }
    if is_third_party_connectivity_ok(settings.third_party_connectivity.as_deref()) {
        return "Ready".into();
    }
    match settings.third_party_connectivity.as_deref() {
        Some(value) if is_connectivity_success(value) => "Ready".into(),
        Some(_) => CONNECTIVITY_FAILED.into(),
        None => {
            if let Some(entry) = selected_model {
                match connectivity_status_label(&entry.metadata).as_deref() {
                    Some(CONNECTIVITY_SUCCESS) => "Ready".into(),
                    Some(CONNECTIVITY_FAILED) => CONNECTIVITY_FAILED.into(),
                    _ => "Setup Required".into(),
                }
            } else {
                "Setup Required".into()
            }
        }
    }
}

pub fn format_health_check_timestamp(timestamp: time::OffsetDateTime) -> String {
    timestamp
        .format(&time::macros::format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second] UTC"
        ))
        .unwrap_or_else(|_| timestamp.to_string())
}

pub fn settings_to_dto(
    settings: &AiInferenceSettings,
    models: &[ModelEntry],
) -> AiInferenceSettingsDto {
    settings_to_dto_with_connectivity_test(settings, models, None)
}

pub fn settings_to_dto_with_connectivity_test(
    settings: &AiInferenceSettings,
    models: &[ModelEntry],
    connectivity_test: Option<(bool, String)>,
) -> AiInferenceSettingsDto {
    let third_party = sort_third_party_models(
        models.iter().filter(|e| is_third_party_model(e)).collect(),
        settings.selected_model_id.as_deref(),
        settings.third_party_connectivity.as_deref(),
    );
    let local = sort_local_models(models.iter().filter(|e| is_local_model(e)).collect());

    let selected_model_name = settings
        .selected_model_id
        .as_ref()
        .and_then(|id| models.iter().find(|m| &m.id == id).map(|m| m.display_model_name()));

    let third_party_available = third_party.iter().any(|m| m.configured);
    let local_available = local.iter().any(|m| m.configured);

    let message = if !settings.initialized {
        "Choose Third-party Providers or Local Runtime to configure AI features".into()
    } else {
        match settings.route {
        AiInferenceRoute::ThirdParty if third_party_available => {
            "Using third-party cloud API for AI features".into()
        }
        AiInferenceRoute::ThirdParty => {
            "Third-party route selected — add a cloud model in Models".into()
        }
        AiInferenceRoute::Local if local_available => {
            "Using local llama.cpp runtime for AI features".into()
        }
        AiInferenceRoute::Local => {
            "Local route selected — install a GGUF model or repair AI Runtime".into()
        }
    }
    };

    AiInferenceSettingsDto {
        route: settings.route.as_str().into(),
        initialized: settings.initialized,
        selected_model_id: settings.selected_model_id.clone(),
        selected_model_name,
        third_party_available,
        local_available,
        third_party_models: third_party,
        local_models: local,
        message,
        connectivity_test_ok: connectivity_test.as_ref().map(|(ok, _)| *ok),
        connectivity_test_detail: connectivity_test.map(|(_, detail)| detail),
    }
}
