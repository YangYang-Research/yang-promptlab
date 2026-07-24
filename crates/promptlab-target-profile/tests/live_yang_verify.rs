use std::collections::HashMap;

use promptlab_target_profile::{verify_target_profile, TargetProfile};

const PROFILE_JSON: &str = r#"{
  "provider": "generic_http",
  "framework": "openai",
  "method": "POST",
  "baseUrl": "https://api.yyng.icu",
  "path": "/ycre/v1/code-review/github/completions",
  "headers": {
    "Content-Type": "application/json",
    "x-yang-api-token": "Basic eXlwYXRfNGU1MDA5MDNjNTZmYTk0Mzo1N2Q1MWU0Yzk5YWUxYjQ2YTdlNzdkYmNhZGYyZGY3MzEyZWQ3NjIzOTFiMWMyOWY="
  },
  "requestTemplate": "{\n  \"chat_session_id\": \"03e98c64-b59d-11f0-b4ea-f0189880e0ee\",\n  \"agent_name\": \"yang-code-review\",\n  \"model_name\": \"anthropic_claude_sonet_4_5\",\n  \"temperature\": 0.7,\n  \"messages\": [\n    {\n      \"role\": \"user\",\n      \"content\": \"{{PROMPT}}\"\n    }\n  ]\n}",
  "promptPlaceholder": "{{PROMPT}}",
  "modelField": "model",
  "streamingField": "stream",
  "conversationField": "messages",
  "toolField": "tools",
  "attachmentField": null,
  "defaultCapabilities": {
    "supportsStreaming": true,
    "supportsTools": true,
    "supportsConversation": true,
    "supportsAttachments": true,
    "supportsMemory": false,
    "supportsAgent": false
  },
  "verificationStrategy": "openai_chat_completion",
  "verification": {
    "verified": false,
    "verifiedAt": null,
    "provider": "",
    "model": null,
    "capabilities": {
      "supportsStreaming": false,
      "supportsTools": false,
      "supportsConversation": false,
      "supportsAttachments": false,
      "supportsMemory": false,
      "supportsAgent": false
    },
    "responseTimeMs": 0,
    "statusCode": 0,
    "status": "pending",
    "responsePreview": null,
    "errorMessage": null
  }
}"#;

#[tokio::test]
async fn live_verify_yang_api_profile() {
    let profile: TargetProfile = serde_json::from_str(PROFILE_JSON).expect("profile json");
    let mut auth_headers = HashMap::new();
    auth_headers.insert(
        "x-yang-api-token".into(),
        "Basic eXlwYXRfNGU1MDA5MDNjNTZmYTk0Mzo1N2Q1MWU0Yzk5YWUxYjQ2YTdlNzdkYmNhZGYyZGY3MzEyZWQ3NjIzOTFiMWMyOWY=".into(),
    );

    let attempt = verify_target_profile(&profile, auth_headers).await;
    eprintln!("status={}", attempt.console.status_code);
    eprintln!("message={}", attempt.console.message);
    eprintln!(
        "preview={:?}",
        attempt.console.response_preview.as_deref().map(|s| &s[..s.len().min(200)])
    );

    assert!(
        attempt.result.is_ok(),
        "verify failed: {} — console: {:?}",
        attempt.console.message,
        attempt.console
    );
}
