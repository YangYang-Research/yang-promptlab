//! IPC DTOs and helpers for AI runtime configuration (`aisec_inference`).

use aisec_inference::config::{AiRuntimeConfiguration, InferenceMode};
use aisec_models::{ModelEntry, ModelProvider};
use serde::{Deserialize, Serialize};

use crate::third_party_credentials::{
    connectivity_status_label, format_connectivity_value, has_third_party_credentials_metadata,
    is_connectivity_success, short_connectivity_list_label, CONNECTIVITY_FAILED, CONNECTIVITY_SUCCESS,
};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connectivity_test_ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connectivity_test_detail: Option<String>,
}

pub fn mode_to_route(mode: InferenceMode) -> &'static str {
    match mode {
        InferenceMode::ThirdParty => "third_party",
        InferenceMode::Local | InferenceMode::Deterministic => "local",
    }
}

pub fn parse_route(raw: &str) -> Option<InferenceMode> {
    InferenceMode::parse(raw).filter(|mode| *mode != InferenceMode::Deterministic)
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

fn third_party_connectivity(config: &AiRuntimeConfiguration) -> Option<&str> {
    if config.mode == InferenceMode::ThirdParty && !config.health.message.is_empty() {
        Some(config.health.message.as_str())
    } else {
        None
    }
}

fn third_party_last_health_check(config: &AiRuntimeConfiguration) -> Option<&str> {
    if config.mode == InferenceMode::ThirdParty {
        config.health.checked_at.as_deref()
    } else {
        None
    }
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
        "Not Verified".into()
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

pub fn reconcile_config(
    mut config: AiRuntimeConfiguration,
    models: &[ModelEntry],
) -> AiRuntimeConfiguration {
    if !config.initialized {
        return config;
    }

    let third_party = sort_third_party_models(
        models.iter().filter(|e| is_third_party_model(e)).collect(),
        config.selected_model_id.as_deref(),
        third_party_connectivity(&config),
    );
    let local = sort_local_models(models.iter().filter(|e| is_local_model(e)).collect());

    let previous_selected = config.selected_model_id.clone();

    let pool = match config.mode {
        InferenceMode::ThirdParty => &third_party,
        InferenceMode::Local | InferenceMode::Deterministic => &local,
    };

    let selected_valid = config
        .selected_model_id
        .as_ref()
        .and_then(|id| pool.iter().find(|m| &m.id == id));

    if selected_valid.is_none() {
        config.selected_model_id = None;
    }

    if config.mode == InferenceMode::ThirdParty {
        let previous_third_party = previous_selected.as_deref().and_then(|id| {
            third_party
                .iter()
                .find(|model| model.id == id)
                .map(|model| model.id.as_str())
        });
        let selected_third_party = config.selected_model_id.as_deref().and_then(|id| {
            third_party
                .iter()
                .find(|model| model.id == id)
                .map(|model| model.id.as_str())
        });
        if let (Some(previous), Some(selected)) = (previous_third_party, selected_third_party) {
            if previous != selected {
                config.health = Default::default();
            }
        }
    }

    config
}

pub fn apply_third_party_health_check(
    config: &mut AiRuntimeConfiguration,
    checked_at: &str,
    ok: bool,
    latency_ms: u64,
) {
    config.health.message = format_connectivity_value(ok, latency_ms);
    config.health.checked_at = Some(checked_at.to_string());
    config.health.ok = ok;
    config.health.latency_ms = Some(latency_ms);
}

pub fn is_third_party_connectivity_ok(config: &AiRuntimeConfiguration) -> bool {
    if config.mode != InferenceMode::ThirdParty {
        return false;
    }
    config.health.ok || third_party_connectivity(config).is_some_and(is_connectivity_success)
}

pub fn third_party_status_label(
    config: &AiRuntimeConfiguration,
    has_configured_model: bool,
    selected_model: Option<&ModelEntry>,
) -> String {
    if !has_configured_model {
        return "N/A".into();
    }
    if config.selected_model_id.is_none() {
        return "Ready".into();
    }
    if is_third_party_connectivity_ok(config) {
        return "Running".into();
    }
    match third_party_connectivity(config) {
        Some(value) if is_connectivity_success(value) => "Running".into(),
        Some(_) => CONNECTIVITY_FAILED.into(),
        None => {
            if let Some(entry) = selected_model {
                match connectivity_status_label(&entry.metadata).as_deref() {
                    Some(CONNECTIVITY_SUCCESS) => "Running".into(),
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

pub fn config_to_dto(config: &AiRuntimeConfiguration, models: &[ModelEntry]) -> AiInferenceSettingsDto {
    config_to_dto_with_connectivity_test(config, models, None)
}

pub fn config_to_dto_with_connectivity_test(
    config: &AiRuntimeConfiguration,
    models: &[ModelEntry],
    connectivity_test: Option<(bool, String)>,
) -> AiInferenceSettingsDto {
    let third_party = sort_third_party_models(
        models.iter().filter(|e| is_third_party_model(e)).collect(),
        config.selected_model_id.as_deref(),
        third_party_connectivity(config),
    );
    let local = sort_local_models(models.iter().filter(|e| is_local_model(e)).collect());

    let selected_model_name = config
        .selected_model_id
        .as_ref()
        .and_then(|id| models.iter().find(|m| &m.id == id).map(|m| m.display_model_name()));

    let third_party_available = third_party.iter().any(|m| m.configured);
    let local_available = local.iter().any(|m| m.configured);

    let message = if !config.initialized {
        "Choose Third-party Providers or Local Runtime to configure AI features".into()
    } else {
        match config.mode {
            InferenceMode::ThirdParty if third_party_available => {
                "Using third-party cloud API for AI features".into()
            }
            InferenceMode::ThirdParty => {
                "Third-party route selected — add a cloud model in Models".into()
            }
            InferenceMode::Local if local_available => {
                "Using local llama.cpp runtime for AI features".into()
            }
            InferenceMode::Local | InferenceMode::Deterministic => {
                "Local route selected — install a GGUF model or repair AI Runtime".into()
            }
        }
    };

    AiInferenceSettingsDto {
        route: mode_to_route(config.mode).into(),
        initialized: config.initialized,
        selected_model_id: config.selected_model_id.clone(),
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
