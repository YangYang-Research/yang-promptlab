import { describe, expect, it } from "vitest";

import {
  buildTargetDescriptor,
  validateTargetStep,
  type TargetFormState,
} from "@/features/scans/targetDescriptor";

function baseForm(overrides: Partial<TargetFormState> = {}): TargetFormState {
  return {
    url: "https://api.example.com",
    authKind: "none",
    loginUrl: "",
    loginUsername: "",
    loginPassword: "",
    usernameSelector: "#email",
    passwordSelector: "#password",
    submitSelector: "button[type=submit]",
    browserSessionId: null,
    browserSessionReady: false,
    ssoLoginUrl: "",
    ssoSuccessUrlPattern: "",
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

  it("persists username/password playwright config", () => {
    expect(
      buildTargetDescriptor(
        baseForm({
          authKind: "username_password",
          loginUsername: "alice",
          loginPassword: "secret",
          browserSessionId: "session-1",
          browserSessionReady: true,
        }),
      ),
    ).toEqual({
      url: "https://api.example.com",
      auth: {
        kind: "username_password",
        engine: "playwright",
        method: "username_password",
        config: {
          type: "username_password",
          login_url: "https://api.example.com",
          username: "alice",
          password: "secret",
          recording_mode: "interactive",
        },
        session_id: "session-1",
      },
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

  it("persists sso playwright config", () => {
    expect(
      buildTargetDescriptor(
        baseForm({
          authKind: "sso",
          browserSessionId: "session-2",
          browserSessionReady: true,
        }),
      ),
    ).toEqual({
      url: "https://api.example.com",
      auth: {
        kind: "sso",
        engine: "playwright",
        method: "oauth",
        config: {
          type: "oauth",
          login_url: "https://api.example.com",
          recording_mode: "interactive",
          provider: null,
        },
        session_id: "session-2",
      },
    });
  });
});

describe("validateTargetStep", () => {
  it("requires a valid https url", () => {
    expect(validateTargetStep(baseForm({ url: "" }))).toMatch(/required/i);
    expect(validateTargetStep(baseForm({ url: "not-a-url" }))).toMatch(/valid URL/i);
  });

  it("requires username/password playwright fields", () => {
    expect(
      validateTargetStep(baseForm({ authKind: "username_password", loginUsername: "" })),
    ).toMatch(/Username/);
  });

  it("requires recorded browser session for playwright auth", () => {
    expect(
      validateTargetStep(
        baseForm({
          authKind: "username_password",
          loginUsername: "alice",
          loginPassword: "secret",
        }),
      ),
    ).toMatch(/Record a browser login session/);
    expect(validateTargetStep(baseForm({ authKind: "sso" }))).toMatch(
      /Complete browser authentication/,
    );
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
