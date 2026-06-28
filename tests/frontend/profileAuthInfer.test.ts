import { describe, expect, it } from "vitest";

import {
  createInitialTargetForm,
  inferAuthFromProfileHeaders,
} from "@/features/scans/targetDescriptor";
import { createInitialTargetProfile } from "@/features/scans/targetProfile";

describe("inferAuthFromProfileHeaders", () => {
  it("syncs endpoint URL from the target profile", () => {
    const profile = createInitialTargetProfile();
    const current = createInitialTargetForm();

    expect(inferAuthFromProfileHeaders(profile, current)).toEqual({
      url: "https://api.openai.com/v1/chat/completions",
    });
  });

  it("detects Bearer token auth from Authorization header", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      "Content-Type": "application/json",
      Authorization: "Bearer sk-test-token",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toEqual({
      url: "https://api.openai.com/v1/chat/completions",
      authKind: "jwt",
      jwtHeaderName: "Authorization",
      jwtPrefix: "Bearer ",
      jwtToken: "sk-test-token",
    });
  });

  it("detects API key headers such as x-api-key", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      "Content-Type": "application/json",
      "x-api-key": "secret-key",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toEqual({
      url: "https://api.openai.com/v1/chat/completions",
      authKind: "api_key",
      apiKeyHeaderName: "x-api-key",
      apiKeyPrefix: "",
      apiKeyValue: "secret-key",
    });
  });

  it("does not override auth when user already selected a method", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      Authorization: "Bearer sk-test-token",
    });
    const current = createInitialTargetForm();
    current.authKind = "basic";
    current.basicUsername = "user";

    expect(inferAuthFromProfileHeaders(profile, current)).toEqual({
      url: "https://api.openai.com/v1/chat/completions",
    });
  });

  it("detects custom token headers such as x-yang-api-token with Basic value", () => {
    const profile = createInitialTargetProfile();
    profile.baseUrl = "https://api.yyng.icu";
    profile.path = "/ycre/v1/code-review/github/completions";
    profile.headersJson = JSON.stringify({
      "Content-Type": "application/json",
      "x-yang-api-token":
        "Basic eXlwYXRfNGU1MDA5MDNjNTZmYTk0Mzo1N2Q1MWU0Yzk5YWUxYjQ2YTdlNzdkYmNhZGYyZGY3MzEyZWQ3NjIzOTFiMWMyOWY=",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toEqual({
      url: "https://api.yyng.icu/ycre/v1/code-review/github/completions",
      authKind: "api_key",
      apiKeyHeaderName: "x-yang-api-token",
      apiKeyPrefix: "Basic ",
      apiKeyValue:
        "eXlwYXRfNGU1MDA5MDNjNTZmYTk0Mzo1N2Q1MWU0Yzk5YWUxYjQ2YTdlNzdkYmNhZGYyZGY3MzEyZWQ3NjIzOTFiMWMyOWY=",
    });
  });

  it("detects *-key headers such as subscription-key", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      "Content-Type": "application/json",
      "subscription-key": "sub-secret",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toEqual({
      url: "https://api.openai.com/v1/chat/completions",
      authKind: "api_key",
      apiKeyHeaderName: "subscription-key",
      apiKeyPrefix: "",
      apiKeyValue: "sub-secret",
    });
  });

  it("detects generic x-* auth headers", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      "Content-Type": "application/json",
      "x-custom-auth": "token-value",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toMatchObject({
      authKind: "api_key",
      apiKeyHeaderName: "x-custom-auth",
      apiKeyValue: "token-value",
    });
  });

  it("splits Bearer prefix only for Authorization header", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      Authorization: "Bearer my-secret",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toMatchObject({
      authKind: "jwt",
      jwtHeaderName: "Authorization",
      jwtPrefix: "Bearer ",
      jwtToken: "my-secret",
    });
  });

  it("splits Bearer prefix for custom auth headers", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      "x-api-token": "Bearer my-secret",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toMatchObject({
      authKind: "api_key",
      apiKeyHeaderName: "x-api-token",
      apiKeyPrefix: "Bearer ",
      apiKeyValue: "my-secret",
    });
  });

  it("re-hydrates missing api key value from Step 2 headers", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      "x-yang-api-token": "Basic abc123",
    });
    const current = createInitialTargetForm();
    current.authKind = "api_key";
    current.apiKeyHeaderName = "x-yang-api-token";
    current.apiKeyPrefix = "Basic ";
    current.apiKeyValue = "";

    expect(inferAuthFromProfileHeaders(profile, current)).toMatchObject({
      authKind: "api_key",
      apiKeyHeaderName: "x-yang-api-token",
      apiKeyPrefix: "Basic ",
      apiKeyValue: "abc123",
      apiKeyVaultMissing: false,
    });
  });

  it("ignores non-auth x-* tracing headers", () => {
    const profile = createInitialTargetProfile();
    profile.headersJson = JSON.stringify({
      "Content-Type": "application/json",
      "x-request-id": "abc-123",
    });

    expect(inferAuthFromProfileHeaders(profile, createInitialTargetForm())).toEqual({
      url: "https://api.openai.com/v1/chat/completions",
    });
  });
});
