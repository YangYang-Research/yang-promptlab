import type { Target } from "@/shared/types";

/** Authentication methods exposed in the Targets / Scan wizard UI. */
export type TargetAuthKind =
  | "none"
  | "username_password"
  | "sso"
  | "basic"
  | "api_key"
  | "jwt";

export type AuthEngineKind = "none" | "auth_engine" | "playwright";

export type TargetFormState = {
  url: string;
  authKind: TargetAuthKind;
  /** Playwright — form login (AuthEngine `UsernamePassword`). */
  loginUrl: string;
  loginUsername: string;
  loginPassword: string;
  usernameSelector: string;
  passwordSelector: string;
  submitSelector: string;
  browserSessionId: string | null;
  browserSessionReady: boolean;
  /** Playwright — interactive SSO / OAuth flow. */
  ssoLoginUrl: string;
  ssoSuccessUrlPattern: string;
  /** AuthEngine — HTTP Basic (`Authorization: Basic …`). */
  basicUsername: string;
  basicPassword: string;
  /** AuthEngine — API key header. */
  apiKeyHeaderName: string;
  apiKeyValue: string;
  apiKeyPrefix: string;
  /** AuthEngine — configured JWT bearer token. */
  jwtToken: string;
  jwtHeaderName: string;
  jwtPrefix: string;
};

export type TargetDescriptorInput = TargetFormState;

export const AUTH_METHOD_OPTIONS: Array<{
  value: TargetAuthKind;
  label: string;
  engine: AuthEngineKind;
  hint: string;
}> = [
  { value: "none", label: "None", engine: "none", hint: "No authentication" },
  {
    value: "username_password",
    label: "Username / Password",
    engine: "playwright",
    hint: "Browser login via Playwright (form fill + session capture)",
  },
  {
    value: "sso",
    label: "SSO",
    engine: "playwright",
    hint: "Interactive browser login via Playwright (OAuth / OIDC / SAML)",
  },
  {
    value: "basic",
    label: "Basic",
    engine: "auth_engine",
    hint: "HTTP Basic credentials applied on each attack request",
  },
  {
    value: "api_key",
    label: "API Key",
    engine: "auth_engine",
    hint: "Static API key header (AuthEngine credential auth)",
  },
  {
    value: "jwt",
    label: "JWT",
    engine: "auth_engine",
    hint: "Configured JWT bearer token (AuthEngine credential auth)",
  },
];

export function authEngineForKind(kind: TargetAuthKind): AuthEngineKind {
  return AUTH_METHOD_OPTIONS.find((option) => option.value === kind)?.engine ?? "none";
}

export function createInitialTargetForm(): TargetFormState {
  return {
    url: "",
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
  };
}

/** Merge persisted wizard session fields from older auth shapes. */
export function migrateTargetForm(form: Partial<TargetFormState> & Record<string, unknown>): TargetFormState {
  const next = { ...createInitialTargetForm(), ...form };

  const legacyKind = form.authKind as string | undefined;
  if (legacyKind === "basic" && typeof form.username === "string" && !form.basicUsername) {
    next.basicUsername = form.username;
    next.basicPassword = typeof form.password === "string" ? form.password : "";
  }
  if (typeof form.username === "string" && legacyKind === "username_password") {
    next.loginUsername = form.username;
    next.loginPassword = typeof form.password === "string" ? form.password : "";
  }
  if (typeof form.headerName === "string") {
    next.apiKeyHeaderName = form.headerName;
  }
  if (typeof form.headerValue === "string") {
    next.apiKeyValue = form.headerValue;
  }
  if (form.ssoSessionReady === true) {
    next.browserSessionReady = true;
  }

  const allowed: TargetAuthKind[] = [
    "none",
    "username_password",
    "sso",
    "basic",
    "api_key",
    "jwt",
  ];
  if (!allowed.includes(next.authKind)) {
    next.authKind = "none";
  }

  return next;
}

export function deriveTargetName(url: string): string {
  const trimmed = url.trim();
  try {
    return new URL(trimmed).hostname || trimmed;
  } catch {
    return trimmed;
  }
}

function validateUrl(url: string): string | null {
  if (!url.trim()) return "Target URL is required.";
  try {
    const parsed = new URL(url.trim());
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "URL must use http or https.";
    }
  } catch {
    return "Enter a valid URL (e.g. https://api.example.com).";
  }
  return null;
}

export function validateTargetStep(input: TargetDescriptorInput): string | null {
  const urlError = validateUrl(input.url);
  if (urlError) return urlError;

  switch (input.authKind) {
    case "username_password":
      if (!input.loginUsername.trim()) return "Username is required.";
      if (!input.loginPassword) return "Password is required.";
      if (!input.browserSessionReady) return "Record a browser login session before continuing.";
      return null;
    case "sso":
      if (!input.browserSessionReady) return "Complete browser authentication before continuing.";
      return null;
    case "basic":
      if (!input.basicUsername.trim()) return "Username is required for Basic auth.";
      if (!input.basicPassword) return "Password is required for Basic auth.";
      return null;
    case "api_key":
      if (!input.apiKeyHeaderName.trim()) return "Header name is required for API key auth.";
      if (!input.apiKeyValue.trim()) return "API key value is required.";
      return null;
    case "jwt":
      if (!input.jwtToken.trim()) return "JWT token is required.";
      return null;
    default:
      return null;
  }
}

function buildAuthBlock(input: TargetDescriptorInput): Record<string, unknown> {
  const engine = authEngineForKind(input.authKind);

  switch (input.authKind) {
    case "username_password":
      return {
        kind: "username_password",
        engine,
        method: "username_password",
        config: {
          type: "username_password",
          login_url: input.url.trim(),
          username: input.loginUsername.trim(),
          password: input.loginPassword,
          recording_mode: "interactive",
        },
        session_id: input.browserSessionId,
      };
    case "sso":
      return {
        kind: "sso",
        engine,
        method: "oauth",
        config: {
          type: "oauth",
          login_url: input.url.trim(),
          recording_mode: "interactive",
          provider: null,
        },
        session_id: input.browserSessionId,
      };
    case "basic":
      return {
        kind: "basic",
        engine,
        method: "basic",
        config: {
          username: input.basicUsername.trim(),
          password: input.basicPassword,
        },
      };
    case "api_key":
      return {
        kind: "api_key",
        engine,
        method: "api_key",
        config: {
          type: "api_key",
          key: input.apiKeyValue.trim(),
          header_name: input.apiKeyHeaderName.trim(),
          prefix: input.apiKeyPrefix.trim() ? input.apiKeyPrefix : null,
        },
      };
    case "jwt":
      return {
        kind: "jwt",
        engine,
        method: "jwt",
        config: {
          type: "jwt",
          token: input.jwtToken.trim(),
          header_name: input.jwtHeaderName.trim() || null,
          prefix: input.jwtPrefix.trim() ? input.jwtPrefix : null,
        },
      };
    default:
      return { kind: "none", engine: "none", method: "none" };
  }
}

/** Build `descriptor_json` persisted by `target_create` (url + auth). */
export function buildTargetDescriptor(input: TargetDescriptorInput): Record<string, unknown> {
  return {
    url: input.url.trim(),
    auth: buildAuthBlock(input),
  };
}

export function targetFormFingerprint(form: TargetFormState): string {
  return JSON.stringify(form);
}

export function targetFormMatchesDescriptor(
  savedTarget: Target,
  form: TargetFormState,
  fingerprint: string,
  savedFingerprint: string | null,
): boolean {
  if (savedTarget.url.trim() !== form.url.trim()) return false;
  return fingerprint === savedFingerprint;
}
