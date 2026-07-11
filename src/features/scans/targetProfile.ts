export const PROMPT_PLACEHOLDER = "{{PROMPT}}";

export type TargetProviderId =
  | "openai_compatible"
  | "openrouter"
  | "anthropic_claude"
  | "google_gemini"
  | "azure_openai"
  | "aws_bedrock"
  | "github_copilot"
  | "open_webui"
  | "dify"
  | "langflow"
  | "mcp"
  | "generic_http"
  | "generic_websocket";

export type TargetCapabilitiesForm = {
  supportsStreaming: boolean;
  supportsTools: boolean;
  supportsConversation: boolean;
  supportsAttachments: boolean;
  supportsMemory: boolean;
  supportsAgent: boolean;
};

export type VerificationResultForm = {
  verified: boolean;
  verifiedAt: string | null;
  provider: string;
  model: string | null;
  capabilities: TargetCapabilitiesForm;
  responseTimeMs: number;
  statusCode: number;
  status: string;
  responsePreview: string | null;
  errorMessage: string | null;
};

export type TargetProfileFormState = {
  provider: TargetProviderId;
  framework: string;
  method: string;
  baseUrl: string;
  path: string;
  headersJson: string;
  requestTemplate: string;
  promptPlaceholder: string;
  modelField: string;
  streamingField: string;
  conversationField: string;
  toolField: string;
  attachmentField: string;
  defaultCapabilities: TargetCapabilitiesForm;
  verificationStrategy: string;
  verification: VerificationResultForm;
};

export type TargetProfileDto = {
  provider: string;
  framework: string;
  method: string;
  baseUrl: string;
  path: string;
  headers: Record<string, string>;
  requestTemplate: string;
  promptPlaceholder: string;
  modelField?: string | null;
  streamingField?: string | null;
  conversationField?: string | null;
  toolField?: string | null;
  attachmentField?: string | null;
  defaultCapabilities: TargetCapabilitiesForm;
  verificationStrategy: string;
  verification: VerificationResultForm;
};

export type VerificationConsoleEntryDto = {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: string;
  statusCode: number;
  responseTimeMs: number;
  responsePreview: string | null;
  success: boolean;
  message: string;
  /** Full curl command built for debugging (may include secrets). */
  requestLog?: string | null;
  /** Auth field summary for debugging. */
  authDebug?: string | null;
};

export const PROVIDER_OPTIONS: Array<{ id: TargetProviderId; label: string }> = [
  { id: "openai_compatible", label: "OpenAI Compatible" },
  { id: "openrouter", label: "OpenRouter" },
  { id: "anthropic_claude", label: "Anthropic Claude" },
  { id: "google_gemini", label: "Google Gemini" },
  { id: "azure_openai", label: "Azure OpenAI" },
  { id: "aws_bedrock", label: "AWS Bedrock" },
  { id: "github_copilot", label: "GitHub Copilot" },
  { id: "open_webui", label: "Open WebUI" },
  { id: "dify", label: "Dify" },
  { id: "langflow", label: "Langflow" },
  { id: "mcp", label: "MCP" },
  { id: "generic_http", label: "Generic HTTP API" },
  { id: "generic_websocket", label: "Generic WebSocket" },
];

export function createEmptyVerification(): VerificationResultForm {
  return {
    verified: false,
    verifiedAt: null,
    provider: "",
    model: null,
    capabilities: createEmptyCapabilities(),
    responseTimeMs: 0,
    statusCode: 0,
    status: "pending",
    responsePreview: null,
    errorMessage: null,
  };
}

/** Wizard/DB may carry verifiedAt as RFC3339 string or legacy time-crate array — normalize for IPC. */
export function normalizeVerifiedAt(value: unknown): string | null {
  if (value == null) return null;
  if (typeof value === "string" && value.trim()) return value.trim();
  return null;
}

export type VerificationBadgeState = {
  label: string;
  variant: "success" | "danger" | "warning" | "muted";
};

export function verificationBadgeFromDb(
  verification: VerificationResultForm,
): VerificationBadgeState {
  if (verification.verified) {
    return { label: "Verified", variant: "success" };
  }
  if (verification.status === "failed" || verification.errorMessage) {
    return { label: "Verification failed", variant: "danger" };
  }
  return { label: "Not verified", variant: "warning" };
}

export function normalizeVerification(
  verification: Partial<VerificationResultForm> | undefined,
): VerificationResultForm {
  const base = createEmptyVerification();
  if (!verification) return base;
  return {
    ...base,
    ...verification,
    verifiedAt: normalizeVerifiedAt(verification.verifiedAt),
  };
}

export function createEmptyCapabilities(): TargetCapabilitiesForm {
  return {
    supportsStreaming: false,
    supportsTools: false,
    supportsConversation: false,
    supportsAttachments: false,
    supportsMemory: false,
    supportsAgent: false,
  };
}

export function createInitialTargetProfile(): TargetProfileFormState {
  return {
    provider: "openai_compatible",
    framework: "openai",
    method: "POST",
    baseUrl: "https://api.openai.com",
    path: "/v1/chat/completions",
    headersJson: '{\n  "Content-Type": "application/json"\n}',
    requestTemplate: `{
  "model": "gpt-4o-mini",
  "messages": [
    { "role": "user", "content": "${PROMPT_PLACEHOLDER}" }
  ],
  "stream": false
}`,
    promptPlaceholder: PROMPT_PLACEHOLDER,
    modelField: "model",
    streamingField: "stream",
    conversationField: "messages",
    toolField: "tools",
    attachmentField: "",
    defaultCapabilities: {
      supportsStreaming: true,
      supportsTools: true,
      supportsConversation: true,
      supportsAttachments: true,
      supportsMemory: false,
      supportsAgent: false,
    },
    verificationStrategy: "openai_chat_completion",
    verification: createEmptyVerification(),
  };
}

export function profileFromDto(dto: TargetProfileDto): TargetProfileFormState {
  return {
    provider: dto.provider as TargetProviderId,
    framework: dto.framework,
    method: dto.method,
    baseUrl: dto.baseUrl,
    path: dto.path,
    headersJson: JSON.stringify(dto.headers ?? {}, null, 2),
    requestTemplate: dto.requestTemplate,
    promptPlaceholder: dto.promptPlaceholder || PROMPT_PLACEHOLDER,
    modelField: dto.modelField ?? "",
    streamingField: dto.streamingField ?? "",
    conversationField: dto.conversationField ?? "",
    toolField: dto.toolField ?? "",
    attachmentField: dto.attachmentField ?? "",
    defaultCapabilities: dto.defaultCapabilities ?? createEmptyCapabilities(),
    verificationStrategy: dto.verificationStrategy,
    verification: normalizeVerification(dto.verification),
  };
}

export function profileToPayload(form: TargetProfileFormState): Record<string, unknown> {
  let headers: Record<string, string> = {};
  try {
    const parsed = JSON.parse(form.headersJson) as Record<string, string>;
    headers = parsed ?? {};
  } catch {
    headers = {};
  }

  return {
    provider: form.provider,
    framework: form.framework,
    method: form.method.toUpperCase(),
    baseUrl: form.baseUrl.trim(),
    path: form.path.trim(),
    headers,
    requestTemplate: form.requestTemplate,
    promptPlaceholder: form.promptPlaceholder || PROMPT_PLACEHOLDER,
    modelField: form.modelField || null,
    streamingField: form.streamingField || null,
    conversationField: form.conversationField || null,
    toolField: form.toolField || null,
    attachmentField: form.attachmentField || null,
    defaultCapabilities: form.defaultCapabilities,
    verificationStrategy: form.verificationStrategy,
    verification: normalizeVerification(form.verification),
  };
}

export function validateTargetProfile(form: TargetProfileFormState): string | null {
  const endpoint = formatApiEndpoint(form);
  if (!endpoint.trim()) return "Endpoint is required.";
  try {
    new URL(endpoint);
  } catch {
    return "Enter a valid Endpoint URL (e.g. https://api.openai.com/v1/chat/completions).";
  }
  if (!form.requestTemplate.includes(form.promptPlaceholder || PROMPT_PLACEHOLDER)) {
    return `Request template must contain ${PROMPT_PLACEHOLDER}.`;
  }
  try {
    JSON.parse(form.headersJson);
  } catch {
    return "Headers must be valid JSON.";
  }
  try {
    JSON.parse(form.requestTemplate);
  } catch {
    if (form.provider !== "generic_http" && form.provider !== "generic_websocket") {
      return "Request template must be valid JSON.";
    }
  }
  return null;
}

export function fullProfileUrl(form: TargetProfileFormState): string {
  const base = form.baseUrl.trim().replace(/\/$/, "");
  const path = form.path.startsWith("/") ? form.path : `/${form.path}`;
  return `${base}${path}`;
}

/** Full endpoint URL shown in the wizard (base + path). */
export function formatApiEndpoint(form: TargetProfileFormState): string {
  if (!form.baseUrl.trim()) return "";
  return fullProfileUrl(form);
}

/** Split a full endpoint URL into base URL and path for backend storage. */
export function splitApiEndpoint(endpoint: string): { baseUrl: string; path: string } {
  const trimmed = endpoint.trim();
  if (!trimmed) {
    return { baseUrl: "", path: "/" };
  }
  try {
    const url = new URL(trimmed);
    const baseUrl = `${url.protocol}//${url.host}`;
    const path = `${url.pathname}${url.search}` || "/";
    return { baseUrl, path };
  } catch {
    return { baseUrl: trimmed.replace(/\/$/, ""), path: "/" };
  }
}

export function applyApiEndpointToProfile(endpoint: string): Partial<TargetProfileFormState> {
  const { baseUrl, path } = splitApiEndpoint(endpoint);
  return { baseUrl, path };
}

export function deriveTargetNameFromProfile(form: TargetProfileFormState): string {
  try {
    const url = new URL(form.baseUrl.trim());
    return url.hostname;
  } catch {
    return form.provider.replace(/_/g, " ");
  }
}

export function formatProviderLabel(providerId: string): string {
  const match = PROVIDER_OPTIONS.find((option) => option.id === providerId);
  if (match) return match.label;
  return providerId.replace(/_/g, " ");
}

export function extractTargetProviderLabel(profile: unknown): string | null {
  if (typeof profile !== "object" || profile === null || Array.isArray(profile)) {
    return null;
  }
  const provider = (profile as Record<string, unknown>).provider;
  if (typeof provider !== "string" || !provider.trim()) {
    return null;
  }
  return formatProviderLabel(provider.trim());
}

export function targetDisplayType(target: {
  type: string;
  providerLabel: string | null;
}): string {
  if (target.providerLabel) return target.providerLabel;
  if (target.type === "llm") return "LLM";
  return target.type.charAt(0).toUpperCase() + target.type.slice(1);
}
