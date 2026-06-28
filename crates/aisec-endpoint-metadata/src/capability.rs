use crate::types::{EndpointCapabilities, EndpointType, FingerprintMetadata, SchemaMetadata};

pub struct CapabilityDetector;

impl CapabilityDetector {
    pub fn detect(
        url: &str,
        fingerprint: &FingerprintMetadata,
        schema: &SchemaMetadata,
        stack_tools: bool,
        stack_memory: bool,
        stack_mcp: bool,
    ) -> EndpointCapabilities {
        let path = url.to_ascii_lowercase();
        let mut caps = EndpointCapabilities::default();

        caps.supports_chat = path.contains("chat")
            || path.contains("completions")
            || path.contains("messages")
            || fingerprint.api_style == "openai_compatible"
            || fingerprint.api_style == "anthropic_messages";

        caps.supports_embedding = path.contains("embedding") || path.contains("embed");

        caps.supports_streaming = schema
            .request_schema
            .as_ref()
            .map(|s| s.fields.iter().any(|f| f.name.eq_ignore_ascii_case("stream")))
            .unwrap_or(path.contains("stream"));

        caps.supports_vision = path.contains("vision")
            || path.contains("image")
            || schema
                .request_schema
                .as_ref()
                .map(|s| {
                    s.fields.iter().any(|f| {
                        matches!(
                            f.name.to_ascii_lowercase().as_str(),
                            "image" | "images" | "attachments"
                        )
                    })
                })
                .unwrap_or(false);

        caps.supports_tools = stack_tools
            || schema
                .request_schema
                .as_ref()
                .map(|s| {
                    s.fields.iter().any(|f| {
                        matches!(
                            f.name.to_ascii_lowercase().as_str(),
                            "tools" | "functions" | "tool_choice"
                        )
                    })
                })
                .unwrap_or(false);

        caps.supports_json_mode = schema
            .request_schema
            .as_ref()
            .map(|s| {
                s.fields.iter().any(|f| {
                    f.name.eq_ignore_ascii_case("response_format")
                        || f.name.eq_ignore_ascii_case("json_mode")
                })
            })
            .unwrap_or(false);

        caps.supports_thinking = path.contains("thinking") || path.contains("reasoning");

        caps.supports_memory = stack_memory
            || path.contains("memory")
            || path.contains("conversation");

        caps.supports_agent = stack_mcp
            || path.contains("agent")
            || path.contains("workflow")
            || caps.supports_tools && caps.supports_memory;

        caps
    }
}

pub fn endpoint_type_hints(caps: &EndpointCapabilities, url: &str, kind: &str) -> EndpointType {
    let path = url.to_ascii_lowercase();
    if kind == "graphql" {
        return EndpointType::UnknownAi;
    }
    if path.contains("mcp") || kind.contains("mcp") {
        return EndpointType::Mcp;
    }
    if caps.supports_embedding {
        return EndpointType::Embedding;
    }
    if path.contains("moderation") {
        return EndpointType::Moderation;
    }
    if path.contains("audio") || path.contains("speech") || path.contains("tts") {
        return EndpointType::Speech;
    }
    if path.contains("image") && !caps.supports_chat {
        return EndpointType::ImageGeneration;
    }
    if caps.supports_agent {
        return EndpointType::AiAgent;
    }
    if caps.supports_tools && !caps.supports_chat {
        return EndpointType::ToolEndpoint;
    }
    if path.contains("workflow") {
        return EndpointType::Workflow;
    }
    if caps.supports_chat {
        return EndpointType::AiChat;
    }
    if path.contains("completion") {
        return EndpointType::Completion;
    }
    if kind == "ai_endpoint" || kind == "rest_api" || kind == "openapi" {
        return EndpointType::UnknownAi;
    }
    EndpointType::NonAi
}
