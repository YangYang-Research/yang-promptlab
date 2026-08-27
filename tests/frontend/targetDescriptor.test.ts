import { describe, expect, it } from "vitest";

import {
  buildTargetDescriptor,
  migrateTargetForm,
  validateTargetStep,
  type TargetFormState,
} from "@/features/scans/targetDescriptor";

function baseForm(overrides: Partial<TargetFormState> = {}): TargetFormState {
  return {
    url: "https://api.example.com",
    authKind: "none",
    basicUsername: "",
    basicPassword: "",
    apiKeyHeaderName: "Authorization",
    apiKeyValue: "",
    apiKeyPrefix: "",
    jwtToken: "",
    jwtHeaderName: "Authorization",
    jwtPrefix: "Bearer ",
    ...overrides,
  };
}

describe("buildTargetDescriptor", () => {
  it("persists none auth", () => {
    expect(buildTargetDescriptor(baseForm())).toEqual({
      url: "https://api.example.com",
      auth: { kind: "none", engine: "none", method: "none" },
    });
  });

  it("persists basic auth engine config", () => {
    expect(
      buildTargetDescriptor(
        baseForm({
          authKind: "basic",
          basicUsername: "alice",
          basicPassword: "secret",
        }),
      ),
    ).toEqual({
      url: "https://api.example.com",
      auth: {
        kind: "basic",
        engine: "auth_engine",
        method: "basic",
        config: { username: "alice", password: "secret" },
      },
    });
  });

  it("persists api key auth engine config", () => {
    expect(
      buildTargetDescriptor(
        baseForm({
          authKind: "api_key",
          apiKeyHeaderName: "X-API-Key",
          apiKeyValue: "sk-test",
        }),
      ),
    ).toEqual({
      url: "https://api.example.com",
      auth: {
        kind: "api_key",
        engine: "auth_engine",
        method: "api_key",
        config: {
          type: "api_key",
          key: "sk-test",
          header_name: "X-API-Key",
          prefix: null,
        },
      },
    });
  });

  it("persists jwt auth engine config", () => {
    expect(
      buildTargetDescriptor(
        baseForm({
          authKind: "jwt",
          jwtToken: "eyJ.test",
        }),
      ),
    ).toMatchObject({
      auth: {
        kind: "jwt",
        engine: "auth_engine",
        config: {
          type: "jwt",
          token: "eyJ.test",
          header_name: "Authorization",
          prefix: "Bearer ",
        },
      },
    });
  });
});

describe("migrateTargetForm", () => {
  it("resets removed username/password and sso to none", () => {
    expect(migrateTargetForm({ authKind: "username_password" as never }).authKind).toBe("none");
    expect(migrateTargetForm({ authKind: "sso" as never }).authKind).toBe("none");
  });

  it("keeps selectable auth kinds", () => {
    expect(migrateTargetForm({ authKind: "basic" }).authKind).toBe("basic");
    expect(migrateTargetForm({ authKind: "api_key" }).authKind).toBe("api_key");
    expect(migrateTargetForm({ authKind: "jwt" }).authKind).toBe("jwt");
  });
});

describe("validateTargetStep", () => {
  it("requires a valid https url", () => {
    expect(validateTargetStep(baseForm({ url: "" }))).toMatch(/required/i);
    expect(validateTargetStep(baseForm({ url: "not-a-url" }))).toMatch(/valid URL/i);
  });

  it("requires basic auth fields", () => {
    expect(
      validateTargetStep(baseForm({ authKind: "basic", basicUsername: "", basicPassword: "" })),
    ).toMatch(/Username/);
  });

  it("requires api key fields", () => {
    expect(
      validateTargetStep(
        baseForm({ authKind: "api_key", apiKeyHeaderName: "", apiKeyValue: "" }),
      ),
    ).toMatch(/Header name/);
  });

  it("requires jwt token", () => {
    expect(validateTargetStep(baseForm({ authKind: "jwt", jwtToken: "" }))).toMatch(/JWT token/);
  });
});
