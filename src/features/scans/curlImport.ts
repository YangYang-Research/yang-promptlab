import type { TargetFormState } from "./targetDescriptor";
import {
  PROMPT_PLACEHOLDER,
  applyApiEndpointToProfile,
  createEmptyVerification,
  type TargetProfileFormState,
  type TargetProviderId,
} from "./targetProfile";

export type ParsedCurl = {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: string | null;
};

export type CurlParseResult =
  | { ok: true; parsed: ParsedCurl }
  | { ok: false; error: string };

export type CurlImportResult =
  | { ok: true; patch: Partial<TargetProfileFormState> }
  | { ok: false; error: string };

const CURL_FLAGS_WITHOUT_VALUE = new Set([
  "-g",
  "-G",
  "-k",
  "-L",
  "-s",
  "-S",
  "-v",
  "-i",
  "-I",
  "-O",
  "--compressed",
  "--insecure",
  "--silent",
  "--show-error",
  "--verbose",
  "--include",
  "--head",
  "--location",
]);

const CURL_DATA_FLAGS = new Set([
  "-d",
  "--data",
  "--data-raw",
  "--data-binary",
  "--data-ascii",
  "--data-urlencode",
]);

function isUrlToken(token: string): boolean {
  return /^https?:\/\//i.test(token) || /^wss?:\/\//i.test(token);
}

/** Tokenize a curl command respecting single- and double-quoted segments. */
export function tokenizeCurl(input: string): string[] {
  const tokens: string[] = [];
  let i = 0;

  while (i < input.length) {
    while (i < input.length && /\s/.test(input[i]!)) i += 1;
    if (i >= input.length) break;

    const quote = input[i];
    if (quote === "'" || quote === '"') {
      i += 1;
      let value = "";
      while (i < input.length && input[i] !== quote) {
        if (quote === '"' && input[i] === "\\" && i + 1 < input.length) {
          i += 1;
          value += input[i]!;
        } else {
          value += input[i]!;
        }
        i += 1;
      }
      if (i < input.length) i += 1;
      tokens.push(value);
      continue;
    }

    let value = "";
    while (i < input.length && !/\s/.test(input[i]!)) {
      value += input[i]!;
      i += 1;
    }
    tokens.push(value);
  }

  return tokens;
}

function normalizeCurlInput(raw: string): string {
  return raw
    .trim()
    .replace(/^curl\s+/i, "")
    .replace(/\\\r?\n/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function parseHeaderLine(line: string): [string, string] | null {
  const idx = line.indexOf(":");
  if (idx <= 0) return null;
  const name = line.slice(0, idx).trim();
  const value = line.slice(idx + 1).trim();
  if (!name) return null;
  return [name, value];
}

function consumeFlagValue(tokens: string[], flagIndex: number): [string | null, number] {
  const flag = tokens[flagIndex]?.toLowerCase() ?? "";
  if (CURL_FLAGS_WITHOUT_VALUE.has(flag)) {
    return [null, flagIndex + 1];
  }
  if (flagIndex + 1 >= tokens.length) {
    return [null, flagIndex + 1];
  }
  return [tokens[flagIndex + 1] ?? null, flagIndex + 2];
}

export function parseCurl(raw: string): CurlParseResult {
  const normalized = normalizeCurlInput(raw);
  if (!normalized) {
    return { ok: false, error: "Paste a cURL command." };
  }

  const tokens = tokenizeCurl(normalized);
  if (tokens.length === 0) {
    return { ok: false, error: "Could not parse cURL command." };
  }

  let method = "GET";
  let url = "";
  const headers: Record<string, string> = {};
  const dataParts: string[] = [];
  let useGetWithQuery = false;

  for (let i = 0; i < tokens.length; ) {
    const token = tokens[i]!;
    const lower = token.toLowerCase();

    if (lower === "-x" || lower === "--request") {
      const [value, next] = consumeFlagValue(tokens, i);
      if (value) method = value.toUpperCase();
      i = next;
      continue;
    }

    if (lower === "-h" || lower === "--header") {
      const [value, next] = consumeFlagValue(tokens, i);
      if (value) {
        const parsed = parseHeaderLine(value);
        if (parsed) {
          const [name, headerValue] = parsed;
          headers[name] = headerValue;
        }
      }
      i = next;
      continue;
    }

    if (CURL_DATA_FLAGS.has(lower)) {
      const [value, next] = consumeFlagValue(tokens, i);
      if (value) {
        if (value.startsWith("@")) {
          return { ok: false, error: "File uploads (@file) are not supported. Paste the request body directly." };
        }
        dataParts.push(value);
        if (method === "GET") method = "POST";
      }
      i = next;
      continue;
    }

    if (lower === "--url") {
      const [value, next] = consumeFlagValue(tokens, i);
      if (value) url = value;
      i = next;
      continue;
    }

    if (lower === "-u" || lower === "--user") {
      const [value, next] = consumeFlagValue(tokens, i);
      if (value && !headers.Authorization) {
        headers.Authorization = `Basic ${btoa(value)}`;
      }
      i = next;
      continue;
    }

    if (lower === "-g" || lower === "--get") {
      useGetWithQuery = true;
      method = "GET";
      i += 1;
      continue;
    }

    if (CURL_FLAGS_WITHOUT_VALUE.has(lower)) {
      i += 1;
      continue;
    }

    if (isUrlToken(token)) {
      url = token;
      i += 1;
      continue;
    }

    i += 1;
  }

  if (!url) {
    return { ok: false, error: "No URL found in cURL command." };
  }

  let body = dataParts.length > 0 ? dataParts.join("&") : null;
  if (useGetWithQuery && body) {
    const joiner = url.includes("?") ? "&" : "?";
    url = `${url}${joiner}${body}`;
    body = null;
  }

  if (body && method === "GET") {
    method = "POST";
  }

  if (method !== "GET" && method !== "POST") {
    return { ok: false, error: `Unsupported HTTP method "${method}". Use GET or POST.` };
  }

  return {
    ok: true,
    parsed: { method, url, headers, body },
  };
}

function inferProvider(url: string, body: string | null): TargetProviderId {
  const u = url.toLowerCase();
  const b = (body ?? "").toLowerCase();

  if (u.includes("api.anthropic.com")) return "anthropic_claude";
  if (u.includes("openrouter.ai")) return "openrouter";
  if (u.includes("generativelanguage.googleapis.com")) return "google_gemini";
  if (u.includes(".openai.azure.com")) return "azure_openai";
  if (u.includes("bedrock-runtime")) return "aws_bedrock";
  if (u.includes("githubcopilot.com")) return "github_copilot";
  if (u.includes("api.dify.ai") || b.includes('"response_mode"')) return "dify";
  if (u.includes("/api/v1/run/") || b.includes('"input_value"')) return "langflow";
  if (b.includes('"jsonrpc"') && (u.includes("/mcp") || b.includes("tools/call"))) return "mcp";
  if (u.includes("/api/chat/completions") && (u.includes("localhost") || u.includes("127.0.0.1"))) {
    return "open_webui";
  }
  if (u.startsWith("wss://") || u.startsWith("ws://")) return "generic_websocket";
  if (u.includes("chat/completions") || u.includes("api.openai.com")) return "openai_compatible";
  return "generic_http";
}

function replacePromptInValue(value: unknown): { value: unknown; found: boolean } {
  if (Array.isArray(value)) {
    for (let i = 0; i < value.length; i += 1) {
      const item = value[i];
      const replaced = replacePromptInValue(item);
      if (replaced.found) {
        const next = [...value];
        next[i] = replaced.value;
        return { value: next, found: true };
      }
    }
    return { value, found: false };
  }

  if (!value || typeof value !== "object") {
    return { value, found: false };
  }

  const obj = value as Record<string, unknown>;

  if (typeof obj.query === "string") {
    return { value: { ...obj, query: PROMPT_PLACEHOLDER }, found: true };
  }
  if (typeof obj.input_value === "string") {
    return { value: { ...obj, input_value: PROMPT_PLACEHOLDER }, found: true };
  }
  if (typeof obj.prompt === "string") {
    return { value: { ...obj, prompt: PROMPT_PLACEHOLDER }, found: true };
  }
  if (typeof obj.input === "string") {
    return { value: { ...obj, input: PROMPT_PLACEHOLDER }, found: true };
  }

  if (Array.isArray(obj.messages)) {
    const messages = obj.messages.map((entry) => {
      if (!entry || typeof entry !== "object") return entry;
      const message = entry as Record<string, unknown>;
      if (typeof message.content === "string") {
        return { ...message, content: PROMPT_PLACEHOLDER };
      }
      if (Array.isArray(message.content)) {
        const content = message.content.map((part) => {
          if (part && typeof part === "object" && typeof (part as Record<string, unknown>).text === "string") {
            return { ...(part as Record<string, unknown>), text: PROMPT_PLACEHOLDER };
          }
          return part;
        });
        return { ...message, content };
      }
      return message;
    });
    const found = JSON.stringify(messages).includes(PROMPT_PLACEHOLDER);
    if (found) {
      return { value: { ...obj, messages }, found: true };
    }
  }

  if (Array.isArray(obj.contents)) {
    const contents = obj.contents.map((entry) => {
      if (!entry || typeof entry !== "object") return entry;
      const content = entry as Record<string, unknown>;
      if (!Array.isArray(content.parts)) return entry;
      const parts = content.parts.map((part) => {
        if (part && typeof part === "object" && typeof (part as Record<string, unknown>).text === "string") {
          return { ...(part as Record<string, unknown>), text: PROMPT_PLACEHOLDER };
        }
        return part;
      });
      return { ...content, parts };
    });
    const found = JSON.stringify(contents).includes(PROMPT_PLACEHOLDER);
    if (found) {
      return { value: { ...obj, contents }, found: true };
    }
  }

  if (obj.params && typeof obj.params === "object" && obj.params !== null) {
    const params = obj.params as Record<string, unknown>;
    if (params.arguments && typeof params.arguments === "object" && params.arguments !== null) {
      const args = params.arguments as Record<string, unknown>;
      if (typeof args.prompt === "string") {
        return {
          value: {
            ...obj,
            params: { ...params, arguments: { ...args, prompt: PROMPT_PLACEHOLDER } },
          },
          found: true,
        };
      }
    }
  }

  return { value, found: false };
}

export function bodyToRequestTemplate(body: string | null): string {
  if (!body?.trim()) {
    return `{\n  "prompt": "${PROMPT_PLACEHOLDER}"\n}`;
  }

  try {
    const parsed = JSON.parse(body) as unknown;
    const replaced = replacePromptInValue(parsed);
    const template = JSON.stringify(replaced.found ? replaced.value : parsed, null, 2);
    if (template.includes(PROMPT_PLACEHOLDER)) {
      return template;
    }
    return `{\n  "prompt": "${PROMPT_PLACEHOLDER}"\n}`;
  } catch {
    if (body.includes(PROMPT_PLACEHOLDER)) return body;
    return body.trim();
  }
}

function inferFieldMapping(body: string | null): Partial<TargetProfileFormState> {
  if (!body?.trim()) return {};

  try {
    const parsed = JSON.parse(body) as Record<string, unknown>;
    const patch: Partial<TargetProfileFormState> = {};
    if ("model" in parsed) patch.modelField = "model";
    if ("stream" in parsed) patch.streamingField = "stream";
    if ("messages" in parsed) patch.conversationField = "messages";
    if ("tools" in parsed) patch.toolField = "tools";
    if ("contents" in parsed) patch.conversationField = "contents";
    if ("parts" in parsed) patch.attachmentField = "parts";
    if ("query" in parsed) patch.conversationField = "query";
    if ("input_value" in parsed) patch.conversationField = "input_value";
    return patch;
  } catch {
    return {};
  }
}

function filterHeaders(headers: Record<string, string>): Record<string, string> {
  const filtered: Record<string, string> = {};
  for (const [name, value] of Object.entries(headers)) {
    if (name.toLowerCase() === "host") continue;
    filtered[name] = value;
  }
  if (Object.keys(filtered).length === 0) {
    filtered["Content-Type"] = "application/json";
  }
  return filtered;
}

/** Map curl/profile headers into Add Target auth form fields. */
export function targetFormAuthFromHeaders(
  headers: Record<string, string>,
): Partial<TargetFormState> {
  const entries = Object.entries(headers);
  const authorization = entries.find(([name]) => name.toLowerCase() === "authorization");
  if (authorization) {
    const value = authorization[1] ?? "";
    if (/^bearer\s+/i.test(value)) {
      return {
        authKind: "jwt",
        jwtToken: value.replace(/^bearer\s+/i, "").trim(),
        jwtHeaderName: "Authorization",
        jwtPrefix: "Bearer ",
        jwtVaultMissing: false,
      };
    }
    if (/^basic\s+/i.test(value)) {
      try {
        const decoded = atob(value.replace(/^basic\s+/i, "").trim());
        const sep = decoded.indexOf(":");
        return {
          authKind: "basic",
          basicUsername: sep >= 0 ? decoded.slice(0, sep) : decoded,
          basicPassword: sep >= 0 ? decoded.slice(sep + 1) : "",
          basicPasswordVaultMissing: false,
        };
      } catch {
        /* fall through */
      }
    }
  }

  const apiKey = entries.find(([name]) => {
    const lower = name.toLowerCase();
    return lower === "x-api-key" || lower === "api-key" || lower === "x-goog-api-key";
  });
  if (apiKey) {
    return {
      authKind: "api_key",
      apiKeyHeaderName: apiKey[0],
      apiKeyValue: apiKey[1],
      apiKeyPrefix: "",
      apiKeyVaultMissing: false,
    };
  }

  return { authKind: "none" };
}

function providerImportDefaults(
  provider: TargetProviderId,
): Pick<TargetProfileFormState, "framework" | "verificationStrategy"> | null {
  switch (provider) {
    case "openrouter":
      return { framework: "openrouter", verificationStrategy: "openai_chat_completion" };
    default:
      return null;
  }
}

export function curlToProfilePatch(raw: string): CurlImportResult {
  const parsedResult = parseCurl(raw);
  if (!parsedResult.ok) {
    return parsedResult;
  }

  const { method, url, headers, body } = parsedResult.parsed;
  const endpointPatch = applyApiEndpointToProfile(url);
  const filteredHeaders = filterHeaders(headers);
  const provider = inferProvider(url, body);
  const requestTemplate = bodyToRequestTemplate(body);
  const fieldMapping = inferFieldMapping(body);
  const providerDefaults = providerImportDefaults(provider);

  if (!requestTemplate.includes(PROMPT_PLACEHOLDER)) {
    return {
      ok: false,
      error: `Could not inject ${PROMPT_PLACEHOLDER} into the request body. Add it manually after import.`,
    };
  }

  return {
    ok: true,
    patch: {
      ...endpointPatch,
      ...fieldMapping,
      ...providerDefaults,
      provider,
      method,
      headersJson: JSON.stringify(filteredHeaders, null, 2),
      requestTemplate,
      verification: createEmptyVerification(),
    },
  };
}
