//! AI endpoint detection demo across all supported providers.
//!
//! Demonstrates the three detection modes:
//!   1. Request patterns  (host / path / method / headers)
//!   2. Response patterns  (status code / JSON body shape)
//!   3. OpenAPI analysis   (servers + operations from a spec)
//!
//! Run: cargo run -p aisec-fingerprint --example detect_ai

use std::collections::HashMap;

use aisec_fingerprint::{AiProvider, FingerprintEngine, FingerprintInput};

fn h(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn obs(
    url: &str,
    method: &str,
    status: u16,
    headers: HashMap<String, String>,
    body: Option<&str>,
) -> FingerprintInput {
    FingerprintInput {
        url: url.into(),
        method: Some(method.into()),
        status: Some(status),
        headers,
        body: body.map(str::to_string),
    }
}

fn main() {
    let engine = FingerprintEngine::new();

    println!("=== AI Endpoint Detection — request + response patterns ===\n");

    // Each case mixes request patterns (host/path/method/headers) and response
    // patterns (status/body) as they would appear from a live probe.
    let cases: Vec<(&str, FingerprintInput)> = vec![
        (
            "OpenAI",
            obs(
                "https://api.openai.com/v1/chat/completions",
                "POST",
                401,
                h(&[("openai-organization", "org-abc")]),
                Some(r#"{"error":{"type":"invalid_request_error","message":"no key"}}"#),
            ),
        ),
        (
            "Anthropic",
            obs(
                "https://api.anthropic.com/v1/messages",
                "POST",
                401,
                h(&[("anthropic-version", "2023-06-01"), ("request-id", "req_1")]),
                Some(r#"{"error":{"type":"authentication_error","message":"x"}}"#),
            ),
        ),
        (
            "Gemini",
            obs(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent",
                "POST",
                200,
                HashMap::new(),
                Some(r#"{"candidates":[{"content":{"parts":[{"text":"hi"}]}}],"promptFeedback":{}}"#),
            ),
        ),
        (
            "Azure OpenAI",
            obs(
                "https://my.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-02-01",
                "POST",
                404,
                h(&[("x-ms-client-request-id", "ms-1")]),
                Some(r#"{"error":{"code":"DeploymentNotFound","message":"x"}}"#),
            ),
        ),
        (
            "Bedrock",
            obs(
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-sonnet/invoke",
                "POST",
                403,
                h(&[("x-amzn-requestid", "abc")]),
                None,
            ),
        ),
        (
            "Ollama",
            obs(
                "http://127.0.0.1:11434/api/tags",
                "GET",
                200,
                HashMap::new(),
                Some(r#"{"models":[{"name":"llama3:latest","model":"llama3:latest"}]}"#),
            ),
        ),
        (
            "LiteLLM",
            obs(
                "https://proxy.internal/v1/chat/completions",
                "POST",
                502,
                h(&[("x-litellm-model-id", "gpt-4"), ("x-litellm-provider", "openai")]),
                Some(r#"{"error":{"type":"litellm_error","message":"x"}}"#),
            ),
        ),
        (
            "vLLM",
            obs(
                "http://gpu-cluster:8000/health",
                "GET",
                200,
                h(&[("server", "uvicorn/vllm-0.6.0")]),
                Some(r#"{"status":"ok","vllm":"0.6.0"}"#),
            ),
        ),
    ];

    for (label, input) in &cases {
        let report = engine.fingerprint(input);
        match report.primary {
            Some(fp) => println!(
                "{label:<14} -> {:<14} confidence={:.2}  method={}  style={:?}  signals={}",
                fp.provider.as_str(),
                fp.confidence,
                fp.suggested_method.as_deref().unwrap_or("-"),
                fp.inferred_api_style,
                fp.signals.len()
            ),
            None => println!("{label:<14} -> (no provider above threshold)"),
        }
    }

    println!("\n=== AI Endpoint Detection — OpenAPI analysis ===\n");

    let specs: Vec<(&str, serde_json::Value)> = vec![
        (
            "OpenAI spec",
            serde_json::json!({
                "openapi": "3.0.0",
                "info": { "title": "OpenAI API", "version": "2.0.0" },
                "servers": [{ "url": "https://api.openai.com/v1" }],
                "paths": {
                    "/chat/completions": { "post": {} },
                    "/models": { "get": {} },
                    "/embeddings": { "post": {} }
                }
            }),
        ),
        (
            "Ollama spec",
            serde_json::json!({
                "openapi": "3.0.0",
                "servers": [{ "url": "http://localhost:11434" }],
                "paths": {
                    "/api/tags": { "get": {} },
                    "/api/generate": { "post": {} },
                    "/api/chat": { "post": {} }
                }
            }),
        ),
        (
            "Anthropic (swagger 2)",
            serde_json::json!({
                "swagger": "2.0",
                "host": "api.anthropic.com",
                "basePath": "/v1",
                "schemes": ["https"],
                "paths": { "/messages": { "post": {} } }
            }),
        ),
    ];

    for (label, spec) in &specs {
        let report = engine.fingerprint_openapi(spec);
        print!("{label:<22} -> ");
        if report.matches.is_empty() {
            println!("(no provider detected)");
            continue;
        }
        let summary: Vec<String> = report
            .matches
            .iter()
            .map(|m| format!("{} ({:.2})", m.provider.as_str(), m.confidence))
            .collect();
        println!("{}", summary.join(", "));
    }

    // Confirm every supported provider has detection coverage.
    println!("\nSupported providers: {} total", AiProvider::all().len());
}
