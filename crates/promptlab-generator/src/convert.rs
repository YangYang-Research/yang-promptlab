use promptlab_attack::{stamp_payload_canary, AttackCategory, AttackPayload, PayloadFormat};
use promptlab_payload::{GeneratedPayload, PayloadCategory, PayloadRecord};

pub fn attack_to_payload_category(category: AttackCategory) -> PayloadCategory {
    match category {
        AttackCategory::PromptInjection => PayloadCategory::PromptInjection,
        AttackCategory::SystemPromptExtraction => PayloadCategory::SystemPromptExtraction,
        AttackCategory::Jailbreak => PayloadCategory::Jailbreak,
        AttackCategory::RagLeakage => PayloadCategory::RagLeakage,
        AttackCategory::MemoryPoisoning => PayloadCategory::MemoryPoisoning,
        AttackCategory::CrossUserLeakage => PayloadCategory::CrossUserLeakage,
        AttackCategory::AgentGoalHijacking => PayloadCategory::AgentGoalHijacking,
        AttackCategory::ToolAbuse => PayloadCategory::ToolAbuse,
        AttackCategory::McpAbuse => PayloadCategory::McpAbuse,
    }
}

pub fn record_to_attack_payload(record: &PayloadRecord) -> AttackPayload {
    let category = payload_to_attack_category(record.category);
    let mut payload = AttackPayload {
        id: record.id.clone(),
        name: record.name.clone(),
        category,
        content: record.content.clone(),
        format: PayloadFormat::Plain,
        metadata: serde_json::json!({
            "tags": record.tags,
            "description": record.description,
            "source": "static_pack",
        })
        .as_object()
        .cloned()
        .map(|m| m.into_iter().collect())
        .unwrap_or_default(),
    };
    stamp_payload_canary(&mut payload);
    payload
}

pub fn generated_to_attack_payload(variant: &GeneratedPayload) -> AttackPayload {
    let category = payload_to_attack_category(variant.category);
    let id = if variant.mutations.is_empty() {
        variant.source_id.clone()
    } else {
        format!("{}:{}", variant.source_id, variant.generation_id)
    };
    let mut payload = AttackPayload {
        id,
        name: variant.source_name.clone(),
        category,
        content: variant.content.clone(),
        format: PayloadFormat::Plain,
        metadata: serde_json::json!({
            "source_id": variant.source_id,
            "generation_id": variant.generation_id,
            "mutations": variant.mutations,
            "source": "template_mutation",
        })
        .as_object()
        .cloned()
        .map(|m| m.into_iter().collect())
        .unwrap_or_default(),
    };
    stamp_payload_canary(&mut payload);
    payload
}

pub fn llm_payload_to_attack(
    category: AttackCategory,
    id: impl Into<String>,
    name: impl Into<String>,
    content: impl Into<String>,
) -> AttackPayload {
    let mut payload = AttackPayload {
        id: id.into(),
        name: name.into(),
        category,
        content: content.into(),
        format: PayloadFormat::Plain,
        metadata: serde_json::json!({ "source": "local_llm" })
            .as_object()
            .cloned()
            .map(|m| m.into_iter().collect())
            .unwrap_or_default(),
    };
    stamp_payload_canary(&mut payload);
    payload
}

fn payload_to_attack_category(category: PayloadCategory) -> AttackCategory {
    match category {
        PayloadCategory::PromptInjection => AttackCategory::PromptInjection,
        PayloadCategory::SystemPromptExtraction => AttackCategory::SystemPromptExtraction,
        PayloadCategory::Jailbreak => AttackCategory::Jailbreak,
        PayloadCategory::RagLeakage => AttackCategory::RagLeakage,
        PayloadCategory::MemoryPoisoning => AttackCategory::MemoryPoisoning,
        PayloadCategory::CrossUserLeakage => AttackCategory::CrossUserLeakage,
        PayloadCategory::AgentGoalHijacking => AttackCategory::AgentGoalHijacking,
        PayloadCategory::ToolAbuse => AttackCategory::ToolAbuse,
        PayloadCategory::McpAbuse => AttackCategory::McpAbuse,
        PayloadCategory::Encoding => AttackCategory::PromptInjection,
    }
}
