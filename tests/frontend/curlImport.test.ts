import { describe, expect, it } from "vitest";

import {
  bodyToRequestTemplate,
  curlToProfilePatch,
  parseCurl,
  tokenizeCurl,
} from "@/features/scans/curlImport";
import { PROMPT_PLACEHOLDER } from "@/features/scans/targetProfile";

describe("tokenizeCurl", () => {
  it("preserves quoted strings with spaces", () => {
    expect(tokenizeCurl('-H "Authorization: Bearer abc"')).toEqual([
      "-H",
      "Authorization: Bearer abc",
    ]);
  });
});

describe("parseCurl", () => {
  it("parses a standard OpenAI cURL", () => {
    const raw = `curl -X POST 'https://api.openai.com/v1/chat/completions' \\
      -H 'Content-Type: application/json' \\
      -H 'Authorization: Bearer sk-test' \\
      -d '{"model":"gpt-4o-mini","messages":[{"role":"user","content":"Hello"}]}'`;

    const result = parseCurl(raw);
    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.parsed.method).toBe("POST");
    expect(result.parsed.url).toBe("https://api.openai.com/v1/chat/completions");
    expect(result.parsed.headers["Content-Type"]).toBe("application/json");
    expect(result.parsed.headers.Authorization).toBe("Bearer sk-test");
    expect(result.parsed.body).toContain("gpt-4o-mini");
  });

  it("rejects file upload bodies", () => {
    const result = parseCurl("curl -d @payload.json https://api.example.com/v1/chat");
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error).toContain("@file");
  });
});

describe("bodyToRequestTemplate", () => {
  it("injects prompt placeholder into OpenAI messages", () => {
    const template = bodyToRequestTemplate(
      '{"model":"gpt-4o-mini","messages":[{"role":"user","content":"Hello"}]}',
    );
    expect(template).toContain(PROMPT_PLACEHOLDER);
    expect(template).not.toContain("Hello");
  });
});

describe("curlToProfilePatch", () => {
  it("maps cURL into target profile form fields", () => {
    const raw = `curl -X POST https://api.anthropic.com/v1/messages \\
      -H 'Content-Type: application/json' \\
      -H 'x-api-key: test' \\
      -H 'anthropic-version: 2023-06-01' \\
      -d '{"model":"claude-3-5-sonnet-20241022","max_tokens":256,"messages":[{"role":"user","content":"Hi"}]}'`;

    const result = curlToProfilePatch(raw);
    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.patch.provider).toBe("anthropic_claude");
    expect(result.patch.method).toBe("POST");
    expect(result.patch.baseUrl).toBe("https://api.anthropic.com");
    expect(result.patch.path).toBe("/v1/messages");
    expect(result.patch.headersJson).toContain("anthropic-version");
    expect(result.patch.requestTemplate).toContain(PROMPT_PLACEHOLDER);
    expect(result.patch.conversationField).toBe("messages");
    expect(result.patch.verification?.verified).toBe(false);
  });

  it("maps OpenRouter cURL into openrouter provider", () => {
    const raw = `curl https://openrouter.ai/api/v1/chat/completions \\
      -H "Content-Type: application/json" \\
      -H "Authorization: Bearer sk-or-test" \\
      -d '{
        "model": "google/gemini-2.5-flash-lite",
        "messages": [{ "role": "user", "content": "What is the meaning of life?" }]
      }'`;

    const result = curlToProfilePatch(raw);
    expect(result.ok).toBe(true);
    if (!result.ok) return;

    expect(result.patch.provider).toBe("openrouter");
    expect(result.patch.framework).toBe("openrouter");
    expect(result.patch.baseUrl).toBe("https://openrouter.ai");
    expect(result.patch.path).toBe("/api/v1/chat/completions");
    expect(result.patch.requestTemplate).toContain(PROMPT_PLACEHOLDER);
    expect(result.patch.requestTemplate).toContain("google/gemini-2.5-flash-lite");
  });
});
