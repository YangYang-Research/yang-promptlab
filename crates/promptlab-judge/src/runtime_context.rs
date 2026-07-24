//! Runtime context passed from the desktop shell into the judge factory.

use aisec_runtime::SharedModelProvider;

/// Runtime bridge for local judge modes.
///
/// Judge → ModelProvider → AISec Runtime → Model
#[derive(Clone)]
pub struct JudgeRuntimeContext {
    pub model_provider: SharedModelProvider,
    pub active_model_id: String,
}

impl JudgeRuntimeContext {
    pub fn new(model_provider: SharedModelProvider, active_model_id: impl Into<String>) -> Self {
        Self {
            model_provider,
            active_model_id: active_model_id.into(),
        }
    }
}
