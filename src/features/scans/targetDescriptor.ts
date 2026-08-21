import type { Target } from "@/shared/types";

import { fullProfileUrl, type TargetProfileFormState } from "./targetProfile";

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
  /** Set when a keychain reference existed but the secret is missing — user must re-enter. */
  apiKeyVaultMissing?: boolean;
  jwtVaultMissing?: boolean;
  basicPasswordVaultMissing?: boolean;
};

export type TargetDescriptorInput = TargetFormState;

export const AUTH_METHOD_OPTIONS: Array<{
  value: TargetAuthKind;
  label: string;
  engine: AuthEngineKind;
  hint: string;
  /** When true, shown in Targets / Scan wizard but not selectable. */
  disabled?: boolean;
}> = [
  { value: "none", label: "None", engine: "none", hint: "No authentication" },
  {
    value: "username_password",
    label: "Username / Password",
    engine: "playwright",
    hint: "Temporarily unavailable",
    disabled: true,
  },
  {
    value: "sso",
    label: "SSO",
    engine: "playwright",
    hint: "Temporarily unavailable",
    disabled: true,
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

/** Auth kinds currently selectable in Targets / Scan wizard. */
export function selectableAuthKinds(): TargetAuthKind[] {
  return AUTH_METHOD_OPTIONS.filter((option) => !option.disabled).map((option) => option.value);
}

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
    apiKeyVaultMissing: false,
    jwtVaultMissing: false,
    basicPasswordVaultMissing: false,
  };
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** Hydrate wizard target form from a persisted target descriptor. */
export function targetFormFromDescriptor(descriptor: unknown, fallbackUrl = ""): TargetFormState {
  const form = createInitialTargetForm();
  const root = asRecord(descriptor);
  const url =
    (typeof root?.url === "string" && root.url) ||
    (typeof root?.base_url === "string" && root.base_url) ||
    fallbackUrl;
  form.url = url;

  const auth = asRecord(root?.auth);
  const kind = typeof auth?.kind === "string" ? auth.kind : "none";
  const config = asRecord(auth?.config);
  const sessionId = typeof auth?.session_id === "string" ? auth.session_id : null;

  switch (kind) {
    case "username_password":
      form.authKind = "username_password";
      form.loginUrl = typeof config?.login_url === "string" ? config.login_url : url;
      form.loginUsername = typeof config?.username === "string" ? config.username : "";
      form.loginPassword = typeof config?.password === "string" ? config.password : "";
      form.browserSessionId = sessionId;
      form.browserSessionReady = Boolean(sessionId);
      break;
    case "sso":
      form.authKind = "sso";
      form.ssoLoginUrl = typeof config?.login_url === "string" ? config.login_url : url;
      form.ssoSuccessUrlPattern =
        typeof config?.success_url_pattern === "string" ? config.success_url_pattern : "";
      form.browserSessionId = sessionId;
      form.browserSessionReady = Boolean(sessionId);
      break;
    case "basic":
      form.authKind = "basic";
      form.basicUsername =
        typeof config?.username === "string"
          ? config.username
          : typeof auth?.username === "string"
            ? auth.username
            : "";
      form.basicPassword = typeof config?.password === "string" ? config.password : "";
      form.basicPasswordVaultMissing = config?.password_vault_missing === true;
      break;
    case "api_key":
      form.authKind = "api_key";
      form.apiKeyHeaderName =
        typeof config?.header_name === "string"
          ? config.header_name
          : typeof auth?.header === "string"
            ? auth.header
            : "Authorization";
      form.apiKeyValue =
        typeof config?.key === "string"
          ? config.key
          : typeof auth?.value === "string"
            ? auth.value
            : "";
      form.apiKeyPrefix = typeof config?.prefix === "string" ? config.prefix : "";
      form.apiKeyVaultMissing = config?.key_vault_missing === true;
      break;
    case "jwt":
      form.authKind = "jwt";
      form.jwtToken = typeof config?.token === "string" ? config.token : "";
      form.jwtHeaderName =
        typeof config?.header_name === "string" ? config.header_name : "Authorization";
      form.jwtPrefix = typeof config?.prefix === "string" ? config.prefix : "Bearer ";
      form.jwtVaultMissing = config?.token_vault_missing === true;
      break;
    default:
      form.authKind = "none";
      break;
  }

  return migrateTargetForm(form);
}

export function targetFormNeedsSecretHydration(form: TargetFormState): boolean {
  switch (form.authKind) {
    case "api_key":
      return !form.apiKeyValue.trim();
    case "jwt":
      return !form.jwtToken.trim();
    case "basic":
      return !form.basicPassword;
    case "username_password":
      return !form.loginPassword;
    default:
      return false;
  }
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

  const allowed = selectableAuthKinds();
  if (!allowed.includes(next.authKind)) {
    next.authKind = "none";
    next.browserSessionReady = false;
    next.browserSessionId = null;
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

const API_KEY_HEADER_NAMES = new Set([
  "x-api-key",
  "api-key",
  "anthropic-api-key",
  "x-goog-api-key",
  "openai-api-key",
]);

const NON_AUTH_HEADER_NAMES = new Set([
  "content-type",
  "accept",
  "user-agent",
  "accept-encoding",
  "accept-language",
  "cache-control",
  "connection",
  "host",
  "origin",
  "referer",
]);

/** Common `x-*` headers that are not credentials. */
const X_HEADER_NON_AUTH_NAMES = new Set([
  "x-request-id",
  "x-correlation-id",
  "x-trace-id",
  "x-forwarded-for",
  "x-forwarded-proto",
  "x-forwarded-host",
  "x-real-ip",
  "x-frame-options",
  "x-content-type-options",
]);

export function isLikelyAuthHeader(name: string): boolean {
  const lower = name.toLowerCase();
  if (NON_AUTH_HEADER_NAMES.has(lower)) return false;
  if (lower === "authorization" || API_KEY_HEADER_NAMES.has(lower)) return true;

  // *-key (api-key, subscription-key, client-key, …)
  if (lower.endsWith("-key")) return true;

  // x-* vendor / gateway auth headers (x-yang-api-token, x-auth-token, …)
  if (lower.startsWith("x-") && !X_HEADER_NON_AUTH_NAMES.has(lower)) return true;

  return (
    /(?:^x-)?(?:api[-_]?)?(?:key|token|auth|credential|secret)/i.test(lower) ||
    lower.includes("api-token") ||
    lower.endsWith("-token")
  );
}

/** Join auth prefix + credential, ensuring scheme prefixes keep a trailing space. */
export function formatCredentialWithPrefix(prefix: string, credential: string): string {
  const secret = credential.trim();
  if (!secret) return "";

  const rawPrefix = prefix;
  const scheme = rawPrefix.trim();
  if (!scheme) return secret;

  if (
    secret.startsWith(rawPrefix) ||
    secret.toLowerCase().startsWith(`${scheme.toLowerCase()} `)
  ) {
    return secret;
  }

  const normalized =
    /^(basic|bearer|token)$/i.test(scheme) ? `${scheme} ` : rawPrefix;
  return `${normalized}${secret}`;
}

function splitCredentialPrefix(value: string): { prefix: string; credential: string } {
  const bearerMatch = value.match(/^(Bearer\s+)(.+)$/i);
  if (bearerMatch) {
    return { prefix: "Bearer ", credential: bearerMatch[2]!.trim() };
  }

  const basicMatch = value.match(/^(Basic\s+)(.+)$/i);
  if (basicMatch) {
    return { prefix: "Basic ", credential: basicMatch[2]!.trim() };
  }

  const tokenMatch = value.match(/^(Token\s+)(.+)$/i);
  if (tokenMatch) {
    return { prefix: "Token ", credential: tokenMatch[2]!.trim() };
  }

  return { prefix: "", credential: value };
}

function inferApiKeyFromHeader(headerName: string, value: string): Partial<TargetFormState> {
  const { prefix, credential } = splitCredentialPrefix(value.trim());
  return {
    authKind: "api_key",
    apiKeyHeaderName: headerName,
    apiKeyPrefix: prefix,
    apiKeyValue: credential,
  };
}

function inferAuthFromHeaderEntry(headerName: string, value: string): Partial<TargetFormState> | null {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const isAuthorization = headerName.toLowerCase() === "authorization";
  const { prefix, credential } = splitCredentialPrefix(trimmed);

  if (prefix.toLowerCase().startsWith("bearer")) {
    if (isAuthorization) {
      return {
        authKind: "jwt",
        jwtHeaderName: headerName,
        jwtPrefix: prefix,
        jwtToken: credential,
      };
    }
    return inferApiKeyFromHeader(headerName, trimmed);
  }

  if (prefix.toLowerCase().startsWith("basic")) {
    if (isAuthorization) {
      try {
        const decoded = atob(credential);
        const colon = decoded.indexOf(":");
        if (colon >= 0) {
          return {
            authKind: "basic",
            basicUsername: decoded.slice(0, colon),
            basicPassword: decoded.slice(colon + 1),
          };
        }
      } catch {
        // Fall through to API key with Basic prefix.
      }
    }
    return inferApiKeyFromHeader(headerName, trimmed);
  }

  if (prefix.toLowerCase().startsWith("token")) {
    return inferApiKeyFromHeader(headerName, trimmed);
  }

  return inferApiKeyFromHeader(headerName, trimmed);
}

function parseProfileHeadersJson(headersJson: string): Record<string, string> {
  try {
    const parsed = JSON.parse(headersJson) as Record<string, string>;
    return parsed ?? {};
  } catch {
    return {};
  }
}

function headerEntries(headers: Record<string, string>): Array<[string, string]> {
  return Object.entries(headers).filter(
    (entry): entry is [string, string] => typeof entry[1] === "string",
  );
}

function findHeader(headers: Record<string, string>, name: string): [string, string] | null {
  const lower = name.toLowerCase();
  for (const [key, value] of headerEntries(headers)) {
    if (key.toLowerCase() === lower) return [key, value];
  }
  return null;
}

function findAuthHeaderForForm(
  headers: Record<string, string>,
  current: TargetFormState,
): [string, string] | null {
  if (current.authKind === "api_key") {
    const name = current.apiKeyHeaderName.trim();
    if (name) {
      const match = findHeader(headers, name);
      if (match) return match;
    }
  }
  if (current.authKind === "jwt") {
    const name = current.jwtHeaderName.trim();
    if (name) {
      const match = findHeader(headers, name);
      if (match) return match;
    }
  }
  if (current.authKind === "basic") {
    const authorization = findHeader(headers, "authorization");
    if (authorization && /^basic\s+/i.test(authorization[1])) {
      return authorization;
    }
    return null;
  }

  const authorization = findHeader(headers, "authorization");
  if (authorization) return authorization;

  for (const [key, value] of headerEntries(headers)) {
    if (key.toLowerCase() === "authorization") continue;
    if (API_KEY_HEADER_NAMES.has(key.toLowerCase()) || isLikelyAuthHeader(key)) {
      return [key, value];
    }
  }

  return null;
}

function mergeInferredAuthFields(
  current: TargetFormState,
  inferred: Partial<TargetFormState>,
): Partial<TargetFormState> {
  switch (current.authKind) {
    case "api_key":
      return {
        authKind: "api_key",
        apiKeyHeaderName: current.apiKeyHeaderName.trim() || inferred.apiKeyHeaderName || "",
        apiKeyPrefix: current.apiKeyPrefix || inferred.apiKeyPrefix || "",
        apiKeyValue: current.apiKeyValue.trim() || inferred.apiKeyValue || "",
        apiKeyVaultMissing: false,
      };
    case "jwt":
      return {
        authKind: "jwt",
        jwtHeaderName: current.jwtHeaderName.trim() || inferred.jwtHeaderName || "Authorization",
        jwtPrefix: current.jwtPrefix || inferred.jwtPrefix || "Bearer ",
        jwtToken: current.jwtToken.trim() || inferred.jwtToken || "",
        jwtVaultMissing: false,
      };
    case "basic":
      return {
        authKind: "basic",
        basicUsername: current.basicUsername.trim() || inferred.basicUsername || "",
        basicPassword: current.basicPassword || inferred.basicPassword || "",
        basicPasswordVaultMissing: false,
      };
    default:
      return inferred;
  }
}

/** Infer wizard auth fields from Step 2 profile headers. */
export function inferAuthFromProfileHeaders(
  profile: TargetProfileFormState,
  current: TargetFormState,
): Partial<TargetFormState> {
  const url = fullProfileUrl(profile);
  const patch: Partial<TargetFormState> = { url };

  const headers = parseProfileHeadersJson(profile.headersJson);

  if (current.authKind !== "none") {
    const needsSecretHydration = targetFormNeedsSecretHydration(current);
    const needsApiKeyPrefix =
      current.authKind === "api_key" && !current.apiKeyPrefix.trim() && !current.apiKeyValue.trim();

    if (needsSecretHydration || needsApiKeyPrefix) {
      const matched = findAuthHeaderForForm(headers, current);
      if (matched) {
        const inferred = inferAuthFromHeaderEntry(matched[0], matched[1]);
        if (inferred && inferred.authKind === current.authKind) {
          return { ...patch, ...mergeInferredAuthFields(current, inferred) };
        }
      }
    }

    return patch;
  }

  const authorization = findHeader(headers, "authorization");

  if (authorization) {
    const inferred = inferAuthFromHeaderEntry(authorization[0], authorization[1]);
    if (inferred) {
      return { ...patch, ...inferred };
    }
  }

  for (const [key, value] of headerEntries(headers)) {
    if (key.toLowerCase() === "authorization") continue;
    if (API_KEY_HEADER_NAMES.has(key.toLowerCase()) || isLikelyAuthHeader(key)) {
      const inferred = inferAuthFromHeaderEntry(key, value);
      if (inferred) {
        return { ...patch, ...inferred };
      }
    }
  }

  return patch;
}

export function syncAuthFormFromProfile(
  profile: TargetProfileFormState,
  current: TargetFormState,
): TargetFormState {
  return { ...current, ...inferAuthFromProfileHeaders(profile, current) };
}

/** Rebuild Step 3 auth fields from Step 2 profile headers (ignores prior Step 3 edits). */
export function inferFreshAuthFormFromProfile(profile: TargetProfileFormState): TargetFormState {
  const profileUrl = fullProfileUrl(profile);
  const blank = { ...createInitialTargetForm(), url: profileUrl };
  return syncAuthFormFromProfile(profile, blank);
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
