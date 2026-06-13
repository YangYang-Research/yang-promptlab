import { describe, expect, it } from "vitest";

import {
  buildTargetDescriptor,
  validateTargetStep,
} from "@/features/scans/targetDescriptor";

describe("buildTargetDescriptor", () => {
  it("persists none auth", () => {
    expect(
      buildTargetDescriptor({ url: "https://api.example.com", authKind: "none" }),
    ).toEqual({
      url: "https://api.example.com",
      auth: { kind: "none" },
    });
  });

  it("persists basic auth credentials", () => {
    expect(
      buildTargetDescriptor({
        url: "https://api.example.com",
        authKind: "basic",
        username: "alice",
        password: "secret",
      }),
    ).toEqual({
      url: "https://api.example.com",
      auth: { kind: "basic", username: "alice", password: "secret" },
    });
  });

  it("persists api key header", () => {
    expect(
      buildTargetDescriptor({
        url: "https://api.example.com",
        authKind: "api_key",
        headerName: "X-API-Key",
        headerValue: "sk-test",
      }),
    ).toEqual({
      url: "https://api.example.com",
      auth: { kind: "api_key", header: "X-API-Key", value: "sk-test" },
    });
  });

  it("persists sso auth placeholder", () => {
    expect(
      buildTargetDescriptor({ url: "https://api.example.com", authKind: "sso" }),
    ).toEqual({
      url: "https://api.example.com",
      auth: { kind: "sso" },
    });
  });
});

describe("validateTargetStep", () => {
  it("requires a valid https url", () => {
    expect(validateTargetStep({ url: "", authKind: "none" })).toMatch(/required/i);
    expect(validateTargetStep({ url: "not-a-url", authKind: "none" })).toMatch(/valid URL/i);
  });

  it("requires basic auth fields", () => {
    expect(
      validateTargetStep({ url: "https://x.com", authKind: "basic", username: "", password: "" }),
    ).toMatch(/Username/);
  });

  it("requires api key fields", () => {
    expect(
      validateTargetStep({
        url: "https://x.com",
        authKind: "api_key",
        headerName: "",
        headerValue: "",
      }),
    ).toMatch(/Header name/);
  });
});
