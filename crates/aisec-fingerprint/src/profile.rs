use crate::types::{
    AgentFramework, AiComponent, FingerprintInput, PlatformProfile, StackFingerprintReport,
};

/// Build the attack-planning platform profile from a stack fingerprint.
pub fn build_platform_profile(
    report: &StackFingerprintReport,
    input: &FingerprintInput,
) -> PlatformProfile {
    let llm_provider = infer_llm_provider(report);
    let platform = infer_platform(report);
    let version = infer_version(report, input);
    let auth_type = infer_auth_type(input);
    let memory_enabled = infer_memory_enabled(report, input);
    let tools_enabled = infer_tools_enabled(report, input);
    let rag_enabled = infer_rag_enabled(report, input);

    PlatformProfile {
        platform,
        version,
        auth_type,
        llm_provider,
        memory_enabled,
        tools_enabled,
        rag_enabled,
    }
}

fn infer_platform(report: &StackFingerprintReport) -> String {
    if let Some(framework) = report.agent_frameworks.first() {
        return framework.framework.as_str().into();
    }

    if report
        .ai_components
        .iter()
        .any(|c| c.component == AiComponent::McpServer)
    {
        return "mcp_server".into();
    }

    if let Some(primary) = report.provider_report.primary.as_ref() {
        return format!("{}_api", primary.provider.as_str());
    }

    if let Some(tech) = report.technologies.first() {
        return format!("{}_api", tech.id);
    }

    String::new()
}

fn infer_llm_provider(report: &StackFingerprintReport) -> String {
    if let Some(primary) = report.provider_report.primary.as_ref() {
        return primary.provider.as_str().into();
    }
    if let Some(tech) = report.technologies.first() {
        return tech.id.clone();
    }

    match report.agent_frameworks.first().map(|f| f.framework) {
        Some(AgentFramework::OpenWebUi) | Some(AgentFramework::LibreChat) => "openai".into(),
        Some(AgentFramework::Dify) | Some(AgentFramework::Flowise) | Some(AgentFramework::Langflow) => {
            "openai".into()
        }
        _ => String::new(),
    }
}

fn infer_version(report: &StackFingerprintReport, input: &FingerprintInput) -> String {
    if let Some(body) = input.body.as_deref() {
        for key in [
            "\"version\"",
            "\"app_version\"",
            "\"build\"",
            "open-webui",
            "LibreChat",
            "langflow",
        ] {
            if body.contains(key) {
                if let Some(v) = extract_json_string_field(body, "version")
                    .or_else(|| extract_json_string_field(body, "app_version"))
                {
                    return v;
                }
            }
        }
    }

    for (header, _) in &input.headers {
        let lower = header.to_ascii_lowercase();
        if lower.contains("version") {
            if let Some(value) = input.headers.get(header) {
                if !value.is_empty() {
                    return value.clone();
                }
            }
        }
    }

    if let Some(framework) = report.agent_frameworks.first() {
        for signal in &framework.signals {
            if signal.to_ascii_lowercase().contains("version") {
                return signal.clone();
            }
        }
    }

    String::new()
}

fn infer_auth_type(input: &FingerprintInput) -> String {
    let headers = &input.headers;
    let header_lower: std::collections::HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();

    if header_lower.contains_key("authorization") {
        let value = header_lower.get("authorization").map(|s| s.to_ascii_lowercase());
        if value.as_deref().is_some_and(|v| v.starts_with("bearer ")) {
            return "bearer".into();
        }
        if value.as_deref().is_some_and(|v| v.starts_with("basic ")) {
            return "basic".into();
        }
        return "authorization_header".into();
    }

    for key in ["x-api-key", "api-key", "x-auth-token", "anthropic-api-key"] {
        if header_lower.contains_key(key) {
            return "api_key".into();
        }
    }

    if let Some(www) = header_lower.get("www-authenticate") {
        if www.to_ascii_lowercase().contains("bearer") {
            return "bearer".into();
        }
        if www.to_ascii_lowercase().contains("basic") {
            return "basic".into();
        }
    }

    if input.status == Some(401) || input.status == Some(403) {
        if let Some(body) = input.body.as_deref() {
            let lower = body.to_ascii_lowercase();
            if lower.contains("api key") || lower.contains("api_key") {
                return "api_key".into();
            }
            if lower.contains("bearer") {
                return "bearer".into();
            }
        }
        return "required".into();
    }

    if input.status == Some(200) {
        return "none".into();
    }

    String::new()
}

fn infer_memory_enabled(report: &StackFingerprintReport, input: &FingerprintInput) -> bool {
    if report
        .agent_frameworks
        .iter()
        .any(|f| matches!(
            f.framework,
            AgentFramework::OpenWebUi
                | AgentFramework::LibreChat
                | AgentFramework::Dify
                | AgentFramework::AnythingLlm
                | AgentFramework::LangChain
                | AgentFramework::LangGraph
        ))
    {
        return true;
    }

    if let Some(body) = input.body.as_deref() {
        let lower = body.to_ascii_lowercase();
        if lower.contains("conversation_id")
            || lower.contains("chat_history")
            || lower.contains("memory")
            || lower.contains("session_id")
        {
            return true;
        }
    }

    input
        .url
        .to_ascii_lowercase()
        .contains("/chats")
        || input.url.to_ascii_lowercase().contains("/threads")
}

fn infer_tools_enabled(report: &StackFingerprintReport, input: &FingerprintInput) -> bool {
    if report
        .ai_components
        .iter()
        .any(|c| matches!(c.component, AiComponent::ToolOrchestration | AiComponent::McpServer))
    {
        return true;
    }

    if report
        .agent_frameworks
        .iter()
        .any(|f| matches!(
            f.framework,
            AgentFramework::LangChain
                | AgentFramework::LangGraph
                | AgentFramework::LangServe
                | AgentFramework::Flowise
                | AgentFramework::Dify
                | AgentFramework::Langflow
                | AgentFramework::CrewAi
                | AgentFramework::AutoGen
        ))
    {
        return true;
    }

    if let Some(body) = input.body.as_deref() {
        let lower = body.to_ascii_lowercase();
        return lower.contains("\"tools\"")
            || lower.contains("function_call")
            || lower.contains("tool_calls");
    }

    false
}

fn infer_rag_enabled(report: &StackFingerprintReport, input: &FingerprintInput) -> bool {
    if report
        .ai_components
        .iter()
        .any(|c| c.component == AiComponent::RagPipeline)
    {
        return true;
    }

    if report.agent_frameworks.iter().any(|f| {
        matches!(
            f.framework,
            AgentFramework::OpenWebUi
                | AgentFramework::Dify
                | AgentFramework::Flowise
                | AgentFramework::Langflow
                | AgentFramework::AnythingLlm
                | AgentFramework::LibreChat
        )
    }) {
        return true;
    }

    if let Some(body) = input.body.as_deref() {
        let lower = body.to_ascii_lowercase();
        return lower.contains("source_documents")
            || lower.contains("retrieved_context")
            || lower.contains("knowledge")
            || lower.contains("vector")
            || lower.contains("embeddings");
    }

    false
}

fn extract_json_string_field(body: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\"");
    let start = body.find(&needle)?;
    let after = &body[start + needle.len()..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ApiStyle, DetectedComponent, DetectedFramework, FingerprintReport, ProviderFingerprint,
    };
    use time::OffsetDateTime;

    fn stack_report(framework: AgentFramework) -> StackFingerprintReport {
        StackFingerprintReport {
            url: "https://example.com".into(),
            confidence: 0.8,
            technologies: vec![],
            agent_frameworks: vec![DetectedFramework {
                framework,
                name: framework.display_name().into(),
                confidence: 0.75,
                signals: vec![],
            }],
            ai_components: vec![],
            provider_report: FingerprintReport {
                url: "https://example.com".into(),
                matches: vec![],
                primary: None,
                analyzed_at: OffsetDateTime::now_utc(),
            },
            attack_recommendations: vec![],
            methods_used: vec![],
            platform_profile: PlatformProfile::default(),
            analyzed_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn openwebui_profile_has_memory_and_rag() {
        let report = stack_report(AgentFramework::OpenWebUi);
        let profile = build_platform_profile(&report, &FingerprintInput::default());
        assert_eq!(profile.platform, "openwebui");
        assert!(profile.memory_enabled);
        assert!(profile.rag_enabled);
    }

    #[test]
    fn mcp_profile_sets_tools() {
        let mut report = stack_report(AgentFramework::LangChain);
        report.agent_frameworks.clear();
        report.ai_components.push(DetectedComponent {
            component: AiComponent::McpServer,
            name: "MCP Server".into(),
            confidence: 0.8,
            signals: vec![],
        });
        let profile = build_platform_profile(&report, &FingerprintInput::default());
        assert_eq!(profile.platform, "mcp_server");
        assert!(profile.tools_enabled);
    }

    #[test]
    fn openai_api_profile() {
        let report = StackFingerprintReport {
            url: "https://api.openai.com/v1/chat/completions".into(),
            confidence: 0.9,
            technologies: vec![],
            agent_frameworks: vec![],
            ai_components: vec![],
            provider_report: FingerprintReport {
                url: "https://api.openai.com/v1/chat/completions".into(),
                matches: vec![ProviderFingerprint {
                    provider: AiProvider::OpenAi,
                    confidence: 0.9,
                    signals: vec![],
                    inferred_api_style: ApiStyle::OpenAiCompatible,
                    suggested_method: None,
                }],
                primary: Some(ProviderFingerprint {
                    provider: AiProvider::OpenAi,
                    confidence: 0.9,
                    signals: vec![],
                    inferred_api_style: ApiStyle::OpenAiCompatible,
                    suggested_method: None,
                }),
                analyzed_at: OffsetDateTime::now_utc(),
            },
            attack_recommendations: vec![],
            methods_used: vec![],
            platform_profile: PlatformProfile::default(),
            analyzed_at: OffsetDateTime::now_utc(),
        };
        let profile = build_platform_profile(&report, &FingerprintInput::default());
        assert_eq!(profile.platform, "openai_api");
        assert_eq!(profile.llm_provider, "openai");
    }
}
