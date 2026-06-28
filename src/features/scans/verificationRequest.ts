import { formatCredentialWithPrefix, type TargetFormState } from "./targetDescriptor";
import {
  PROMPT_PLACEHOLDER,
  fullProfileUrl,
  type TargetProfileFormState,
  type VerificationConsoleEntryDto,
} from "./targetProfile";

const VERIFY_PROMPT = "Hello";

function parseProfileHeaders(headersJson: string): Record<string, string> {
  try {
    const parsed = JSON.parse(headersJson) as Record<string, string>;
    return parsed ?? {};
  } catch {
    return {};
  }
}

/** Build auth headers from the wizard form (mirrors backend verify merge). */
export function authHeadersFromForm(form: TargetFormState): Record<string, string> {
  const headers: Record<string, string> = {};

  switch (form.authKind) {
    case "api_key": {
      const headerName = form.apiKeyHeaderName.trim() || "Authorization";
      const value = formatCredentialWithPrefix(form.apiKeyPrefix, form.apiKeyValue);
      if (value) headers[headerName] = value;
      break;
    }
    case "jwt": {
      const headerName = form.jwtHeaderName.trim() || "Authorization";
      const value = formatCredentialWithPrefix(form.jwtPrefix || "Bearer ", form.jwtToken);
      if (value) headers[headerName] = value;
      break;
    }
    case "basic": {
      if (form.basicUsername.trim() && form.basicPassword) {
        const encoded = btoa(`${form.basicUsername.trim()}:${form.basicPassword}`);
        headers.Authorization = `Basic ${encoded}`;
      }
      break;
    }
    default:
      break;
  }

  return headers;
}

export function buildVerificationBody(profile: TargetProfileFormState): string {
  const placeholder = profile.promptPlaceholder || PROMPT_PLACEHOLDER;
  return profile.requestTemplate.replaceAll(placeholder, VERIFY_PROMPT);
}

export function mergeVerificationHeaders(
  profile: TargetProfileFormState,
  authForm: TargetFormState,
): Record<string, string> {
  const headers = { ...parseProfileHeaders(profile.headersJson) };
  const authHeaders = authHeadersFromForm(authForm);
  for (const [key, value] of Object.entries(authHeaders)) {
    headers[key] = value;
  }
  return headers;
}

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, "'\\''")}'`;
}

export function formatVerificationRequestLog(input: {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: string;
}): string {
  const method = input.method.toUpperCase();
  const lines = [`curl --location ${shellQuote(input.url)} \\`, `  --request ${method} \\`];

  for (const [name, value] of Object.entries(input.headers)) {
    lines.push(`  --header ${shellQuote(`${name}: ${value}`)} \\`);
  }

  if (method !== "GET" && input.body.trim()) {
    const last = lines.length - 1;
    lines[last] = lines[last]!.replace(/ \\$/, "");
    lines.push(`  --data ${shellQuote(input.body)}`);
  } else {
    const last = lines.length - 1;
    lines[last] = lines[last]!.replace(/ \\$/, "");
  }

  return lines.join("\n");
}

export function buildAuthDebugSummary(
  profile: TargetProfileFormState,
  authForm: TargetFormState,
): string {
  const profileHeaderNames = Object.keys(parseProfileHeaders(profile.headersJson));
  const lines = [`Auth kind: ${authForm.authKind}`];

  if (authForm.authKind === "api_key") {
    lines.push(`Header: ${authForm.apiKeyHeaderName || "(empty)"}`);
    lines.push(`Prefix: ${JSON.stringify(authForm.apiKeyPrefix)}`);
    lines.push(`Key length: ${authForm.apiKeyValue.trim().length} chars`);
    if (authForm.apiKeyVaultMissing) {
      lines.push("Keychain: missing — re-enter API key");
    }
  } else if (authForm.authKind === "jwt") {
    lines.push(`Header: ${authForm.jwtHeaderName || "Authorization"}`);
    lines.push(`Prefix: ${JSON.stringify(authForm.jwtPrefix)}`);
    lines.push(`Token length: ${authForm.jwtToken.trim().length} chars`);
  } else if (authForm.authKind === "basic") {
    lines.push(`Username: ${authForm.basicUsername || "(empty)"}`);
    lines.push(`Password length: ${authForm.basicPassword.length} chars`);
  }

  lines.push(`Profile headers (Step 2): ${profileHeaderNames.join(", ") || "(none)"}`);
  lines.push("Form auth overrides matching profile header names at verify time.");

  return lines.join("\n");
}

export function buildVerificationRequestPreview(
  profile: TargetProfileFormState,
  authForm: TargetFormState,
  message = "Sending verification request…",
): VerificationConsoleEntryDto & { requestLog: string; authDebug: string } {
  const method = profile.method.toUpperCase() || "POST";
  const url = fullProfileUrl(profile);
  const headers = mergeVerificationHeaders(profile, authForm);
  const body = buildVerificationBody(profile);
  const requestLog = formatVerificationRequestLog({ method, url, headers, body });

  return {
    method,
    url,
    headers,
    body,
    statusCode: 0,
    responseTimeMs: 0,
    responsePreview: null,
    success: false,
    message,
    requestLog,
    authDebug: buildAuthDebugSummary(profile, authForm),
  };
}
