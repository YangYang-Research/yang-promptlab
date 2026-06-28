use aisec_attack::AttackCategory;
use aisec_fingerprint::{PlatformProfile, StackFingerprintReport};
use serde::{Deserialize, Serialize};

/// Planner execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannerMode {
    Deterministic,
    LocalLlm,
}

/// Fingerprint input for attack planning (one or more probed endpoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintResult {
    pub endpoints: Vec<FingerprintEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintEndpoint {
    pub endpoint_id: String,
    pub url: String,
    pub report: StackFingerprintReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<aisec_endpoint_metadata::AiEndpointMetadata>,
}

impl FingerprintResult {
    pub fn single(endpoint_id: impl Into<String>, url: impl Into<String>, report: StackFingerprintReport) -> Self {
        Self {
            endpoints: vec![FingerprintEndpoint {
                endpoint_id: endpoint_id.into(),
                url: url.into(),
                report,
                metadata: None,
            }],
        }
    }

    pub fn platform_profiles(&self) -> Vec<PlatformProfile> {
        let mut seen = std::collections::HashSet::new();
        let mut profiles = Vec::new();
        for endpoint in &self.endpoints {
            let platform = endpoint.report.platform_profile.platform.clone();
            if platform.is_empty() || !seen.insert(platform.clone()) {
                continue;
            }
            profiles.push(endpoint.report.platform_profile.clone());
        }
        profiles
    }

    pub fn merged_capabilities(&self) -> MergedCapabilities {
        let mut caps = MergedCapabilities::default();
        for endpoint in &self.endpoints {
            if let Some(metadata) = &endpoint.metadata {
                caps.memory_enabled |= metadata.capabilities.supports_memory;
                caps.tools_enabled |= metadata.capabilities.supports_tools;
                caps.rag_enabled |= metadata.capabilities.supports_agent;
                if !metadata.fingerprint.framework.is_empty() {
                    caps.platforms.insert(metadata.fingerprint.framework.clone());
                }
                if metadata.capabilities.supports_agent
                    || metadata.classification.endpoint_type
                        == aisec_endpoint_metadata::EndpointType::Mcp
                {
                    caps.mcp_detected = true;
                }
                caps.has_ai_surface |= metadata.classification.endpoint_type
                    != aisec_endpoint_metadata::EndpointType::NonAi;
                caps.max_risk_score = caps.max_risk_score.max(metadata.risk.score);
                continue;
            }
            let p = &endpoint.report.platform_profile;
            caps.memory_enabled |= p.memory_enabled;
            caps.tools_enabled |= p.tools_enabled;
            caps.rag_enabled |= p.rag_enabled;
            if !p.platform.is_empty() {
                caps.platforms.insert(p.platform.clone());
            }
            if endpoint
                .report
                .ai_components
                .iter()
                .any(|c| c.component == aisec_fingerprint::AiComponent::McpServer)
            {
                caps.mcp_detected = true;
            }
        }
        caps.has_ai_surface = caps.has_ai_surface
            || (!self.endpoints.is_empty()
                && self.endpoints.iter().any(|e| {
                    e.report.confidence > 0.0
                        || !e.report.platform_profile.platform.is_empty()
                        || !e.report.technologies.is_empty()
                }));
        caps
    }
}

#[derive(Debug, Clone, Default)]
pub struct MergedCapabilities {
    pub has_ai_surface: bool,
    pub memory_enabled: bool,
    pub tools_enabled: bool,
    pub rag_enabled: bool,
    pub mcp_detected: bool,
    pub platforms: std::collections::HashSet<String>,
    pub max_risk_score: u8,
}

/// Generated attack plan for the Scan Wizard / scan_start playbook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackPlan {
    pub mode: PlannerMode,
    pub profile_id: String,
    pub categories: Vec<AttackCategory>,
    pub disabled_tests: Vec<String>,
    pub rationales: Vec<CategoryRationale>,
    pub confidence: f32,
    pub summary: String,
    pub llm_rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryRationale {
    pub category: AttackCategory,
    pub reason: String,
    pub priority: u8,
    pub source: String,
}
