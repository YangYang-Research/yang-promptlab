import { describe, expect, it } from "vitest";

import { createInitialTargetForm } from "@/features/scans/targetDescriptor";
import { createInitialTargetProfile } from "@/features/scans/targetProfile";
import { buildVerificationRequestPreview } from "@/features/scans/verificationRequest";
import {
  formatAiValidationLogLine,
  formatAuthenticationLogLines,
  formatResponseLogLine,
  formatSendRequestLogLine,
} from "@/features/scans/verificationLog";

describe("verificationLog", () => {
  it("formats a flat auth and request timeline", () => {
    const profile = createInitialTargetProfile();
    profile.baseUrl = "https://api.yyng.icu";
    profile.path = "/ycre/v1/code-review/github/completions";
    profile.headersJson = JSON.stringify({ "Content-Type": "application/json" });

    const authForm = createInitialTargetForm();
    authForm.authKind = "api_key";
    authForm.apiKeyHeaderName = "x-yang-api-token";
    authForm.apiKeyPrefix = "Basic ";
    authForm.apiKeyValue = "eXlwYXRfNGU1Q3NjIzOTFiMWMyOWY=";

    const preview = buildVerificationRequestPreview(profile, authForm);
    const authLines = formatAuthenticationLogLines(preview.headers);
    const sendLine = formatSendRequestLogLine(preview.requestLog);

    expect(authLines[0]).toMatch(/^Step 1 — Auth header applied: x-yang-api-token = Basic /);
    expect(authLines[0]).toContain("***");
    expect(sendLine).toContain("Step 1 — Outbound probe request:");
    expect(sendLine).toContain("curl --location 'https://api.yyng.icu/ycre/v1/code-review/github/completions'");
    expect(sendLine).toContain("--request POST");
    expect(sendLine).toContain("--header");
  });

  it("shows full target body for connectivity and AI Runtime message for step 2", () => {
    const longBody = '{"answer":"' + "x".repeat(400) + '"}';
    const connectivity = formatResponseLogLine(
      {
        method: "POST",
        url: "https://api.example.com",
        headers: {},
        body: "",
        statusCode: 200,
        responseTimeMs: 4126,
        responsePreview: longBody,
        success: true,
        message: "Authentication and connectivity verified",
      },
      "connectivity",
    );

    expect(connectivity).toContain("Step 1 — Connectivity result: HTTP 200 · 4126ms");
    expect(connectivity).toContain(longBody);
    expect(connectivity).not.toContain("…");

    const aiLine = formatAiValidationLogLine(
      "Verification succeeded — this is an AI API endpoint (confidence 100%): assistant reply",
    );
    expect(aiLine).toBe(
      "Step 2 — AI validation result: Verification succeeded — this is an AI API endpoint (confidence 100%): assistant reply",
    );
    expect(aiLine).not.toContain("HTTP 200");
  });
});
