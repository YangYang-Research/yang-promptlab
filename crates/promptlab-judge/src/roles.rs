use std::sync::Arc;

use aisec_models::runtime::InferenceRuntime;
use tokio::sync::Mutex;

use crate::error::{JudgeError, JudgeResult};
use crate::types::ModelRole;

/// Pool of offline LLM runtimes keyed by model role.
#[derive(Clone)]
pub struct ModelRolePool {
    judge: Option<Arc<Mutex<dyn InferenceRuntime>>>,
    classifier: Option<Arc<Mutex<dyn InferenceRuntime>>>,
    attacker: Option<Arc<Mutex<dyn InferenceRuntime>>>,
}

impl ModelRolePool {
    pub fn new() -> Self {
        Self {
            judge: None,
            classifier: None,
            attacker: None,
        }
    }

    pub fn set_judge(&mut self, runtime: Arc<Mutex<dyn InferenceRuntime>>) {
        self.judge = Some(runtime);
    }

    pub fn set_classifier(&mut self, runtime: Arc<Mutex<dyn InferenceRuntime>>) {
        self.classifier = Some(runtime);
    }

    pub fn set_attacker(&mut self, runtime: Arc<Mutex<dyn InferenceRuntime>>) {
        self.attacker = Some(runtime);
    }

    pub fn get(&self, role: ModelRole) -> JudgeResult<Arc<Mutex<dyn InferenceRuntime>>> {
        let slot = match role {
            ModelRole::Judge => &self.judge,
            ModelRole::Classifier => &self.classifier,
            ModelRole::Attacker => &self.attacker,
        };
        slot.clone()
            .ok_or_else(|| JudgeError::RoleNotConfigured(role.as_str().into()))
    }

    pub fn configured_roles(&self) -> Vec<ModelRole> {
        let mut roles = Vec::new();
        if self.judge.is_some() {
            roles.push(ModelRole::Judge);
        }
        if self.classifier.is_some() {
            roles.push(ModelRole::Classifier);
        }
        if self.attacker.is_some() {
            roles.push(ModelRole::Attacker);
        }
        roles
    }

    /// Register the same runtime for all roles (single-model mode).
    pub fn set_all(&mut self, runtime: Arc<Mutex<dyn InferenceRuntime>>) {
        self.judge = Some(runtime.clone());
        self.classifier = Some(runtime.clone());
        self.attacker = Some(runtime);
    }
}

impl Default for ModelRolePool {
    fn default() -> Self {
        Self::new()
    }
}
