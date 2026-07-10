import { describe, expect, it } from "vitest";

import { createInitialTargetForm } from "@/features/scans/targetDescriptor";
import {
  createInitialTargetProfile,
  PROMPT_PLACEHOLDER,
} from "@/features/scans/targetProfile";
import {
  buildVerificationRequestPreview,
  formatVerificationRequestLog,
  mergeVerificationHeaders,
} from "@/features/scans/verificationRequest";

describe("verificationRequest", () => {
  it("builds a full curl log with merged auth headers", () => {
    const profile = createInitialTargetProfile();
    profile.baseUrl = "https://api.yyng.icu";
    profile.path = "/ycre/v1/code-review/github/completions";
    profile.headersJson = JSON.stringify({ "Content-Type": "application/json" });
    profile.requestTemplate = `{
  "model_name": "anthropic_claude_sonet_4_5",
  "messages": [{ "role": "user", "content": "${PROMPT_PLACEHOLDER}" }]
}`;

    const authForm = createInitialTargetForm();
    authForm.authKind = "api_key";
    authForm.apiKeyHeaderName = "x-yang-api-token";
    authForm.apiKeyPrefix = "Basic ";
    authForm.apiKeyValue = "abc123";

    const preview = buildVerificationRequestPreview(profile, authForm);

    expect(preview.requestLog).toContain("curl --location 'https://api.yyng.icu/ycre/v1/code-review/github/completions'");
    expect(preview.requestLog).toContain("x-yang-api-token: Basic");
    expect(preview.requestLog).toContain("***");
    expect(preview.requestLog).not.toContain("x-yang-api-token: Basic abc123");
    expect(preview.requestLog).toContain("Authorized API capability inventory");
    expect(preview.requestLog).not.toContain('"content": "Hello"');
    expect(preview.authDebug).toContain("api_key");
    expect(mergeVerificationHeaders(profile, authForm)["x-yang-api-token"]).toBe(
      "Basic abc123",
    );
  });

  it("replaces profile credential headers when form auth uses a different header", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      Authorization: "Bearer old-token",
      "Content-Type": "application/json",
    });
    profile.requestTemplate = `{"messages":[{"content":"${PROMPT_PLACEHOLDER}"}]}`;

    const authForm = createInitialTargetForm();
    authForm.authKind = "api_key";
    authForm.apiKeyHeaderName = "x-yang-api-token";
    authForm.apiKeyPrefix = "Basic ";
    authForm.apiKeyValue = "abc123";

    const headers = mergeVerificationHeaders(profile, authForm);
    expect(headers.Authorization).toBeUndefined();
    expect(headers["x-yang-api-token"]).toBe("Basic abc123");
    expect(headers["Content-Type"]).toBe("application/json");
  });

  it("inserts space when prefix is Basic without trailing space", () => {
    const profile = createInitialTargetProfile();
    profile.baseUrl = "https://api.example.com";
    profile.path = "/v1/chat";
    profile.headersJson = "{}";
    profile.requestTemplate = `{"messages":[{"content":"${PROMPT_PLACEHOLDER}"}]}`;

    const authForm = createInitialTargetForm();
    authForm.authKind = "api_key";
    authForm.apiKeyHeaderName = "x-yang-api-token";
    authForm.apiKeyPrefix = "Basic";
    authForm.apiKeyValue = "eXlwYXQ=";

    const preview = buildVerificationRequestPreview(profile, authForm);
    expect(preview.requestLog).toContain("x-yang-api-token: Basic");
    expect(preview.requestLog).toContain("***");
    expect(preview.requestLog).not.toContain("x-yang-api-token: Basic eXlwYXQ=");
    expect(preview.requestLog).not.toContain("BasiceXlwYXQ=");
  });

  it("formats GET requests without a body", () => {
    const log = formatVerificationRequestLog({
      method: "GET",
      url: "https://example.com/health",
      headers: { Accept: "application/json" },
      body: "",
    });

    expect(log).toContain("--request GET");
    expect(log).not.toContain("--data");
  });
});
