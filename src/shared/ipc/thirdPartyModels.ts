import { invokeCommand } from "./invoke";
import { saveThirdPartyModel, type ThirdPartyModelSaveRequest } from "./models";

export type ThirdPartyProvider =
  | "openai"
  | "anthropic"
  | "gemini"
  | "azure"
  | "bedrock"
  | "custom";

export type ThirdPartyModelForm = {
  provider: ThirdPartyProvider;
  customProviderName: string;
  model: string;
  baseUrl: string | null;
  apiKey: string;
  apiKeyEnv: string | null;
  awsSecretAccessKey: string;
  awsSessionToken: string;
  awsRegion: string | null;
  apiKeyConfigured: boolean;
  awsSecretAccessKeyConfigured: boolean;
  awsSessionTokenConfigured: boolean;
};

export type ThirdPartyModelConnectivityResult = {
  ok: boolean;
  provider: string;
  model: string;
  latencyMs: number;
  message: string;
  sampleResponse?: string | null;
};

export type ThirdPartyModelEditDto = {
  provider: string;
  model: string;
  baseUrl: string | null;
  region: string | null;
  apiKeyEnv: string | null;
  apiKeyConfigured: boolean;
  awsSecretAccessKeyConfigured: boolean;
  awsSessionTokenConfigured: boolean;
};

const KNOWN_THIRD_PARTY_PROVIDERS = new Set<ThirdPartyProvider>([
  "openai",
  "anthropic",
  "gemini",
  "azure",
  "bedrock",
]);

export const THIRD_PARTY_PROVIDERS: Array<{
  value: ThirdPartyProvider;
  label: string;
  modelPlaceholder: string;
  apiKeyEnv: string;
  requiresBaseUrl?: boolean;
  baseUrlPlaceholder?: string;
  regionPlaceholder?: string;
}> = [
  {
    value: "openai",
    label: "OpenAI",
    modelPlaceholder: "gpt-4o-mini",
    apiKeyEnv: "OPENAI_API_KEY",
    baseUrlPlaceholder: "https://api.openai.com/v1",
  },
  {
    value: "anthropic",
    label: "Anthropic",
    modelPlaceholder: "claude-sonnet-4-20250514",
    apiKeyEnv: "ANTHROPIC_API_KEY",
    baseUrlPlaceholder: "https://api.anthropic.com/v1",
  },
  {
    value: "gemini",
    label: "Google",
    modelPlaceholder: "gemini-2.0-flash",
    apiKeyEnv: "GOOGLE_API_KEY",
    baseUrlPlaceholder: "https://generativelanguage.googleapis.com/v1beta",
  },
  {
    value: "azure",
    label: "Azure",
    modelPlaceholder: "gpt-4o",
    apiKeyEnv: "AZURE_OPENAI_API_KEY",
    requiresBaseUrl: true,
    baseUrlPlaceholder: "https://{resource}.openai.azure.com/openai/deployments/{deployment}",
  },
  {
    value: "bedrock",
    label: "AWS Bedrock",
    modelPlaceholder: "global.anthropic.claude-haiku-4-5-20251001-v1:0",
    apiKeyEnv: "AWS_ACCESS_KEY_ID",
    regionPlaceholder: "us-east-1",
  },
  {
    value: "custom",
    label: "Custom",
    modelPlaceholder: "my-model",
    apiKeyEnv: "CUSTOM_API_KEY",
    requiresBaseUrl: true,
    baseUrlPlaceholder: "https://your-host/v1",
  },
];

function normalizeCustomProviderSlug(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function thirdPartyModelTemplate(
  provider: ThirdPartyProvider = "openai",
): ThirdPartyModelForm {
  const meta = THIRD_PARTY_PROVIDERS.find((entry) => entry.value === provider);
  return {
    provider,
    customProviderName: "",
    model: "",
    baseUrl: null,
    apiKey: "",
    apiKeyEnv: meta?.apiKeyEnv ?? "OPENAI_API_KEY",
    awsSecretAccessKey: "",
    awsSessionToken: "",
    awsRegion: null,
    apiKeyConfigured: false,
    awsSecretAccessKeyConfigured: false,
    awsSessionTokenConfigured: false,
  };
}

export function validateThirdPartyModelForm(form: ThirdPartyModelForm): string | null {
  if (!form.model.trim()) {
    return "Model name is required";
  }

  if (form.provider === "bedrock") {
    if (!form.apiKey.trim() && !form.apiKeyConfigured) {
      return "Access Key ID is required";
    }
    if (!form.awsSecretAccessKey.trim() && !form.awsSecretAccessKeyConfigured) {
      return "Secret Access Key is required";
    }
    if (!form.awsRegion?.trim()) {
      return "Region is required";
    }
    if (
      form.apiKey.trim().startsWith("ASIA") &&
      !form.awsSessionToken.trim() &&
      !form.awsSessionTokenConfigured
    ) {
      return "Session Token is required for temporary AWS credentials (ASIA access keys)";
    }
    return null;
  }

  if (form.provider === "azure" && !form.baseUrl?.trim()) {
    return "Endpoint URL is required";
  }

  if (form.provider === "custom" && !form.customProviderName.trim()) {
    return "Provider name is required";
  }

  if (form.provider === "custom") {
    const slug = normalizeCustomProviderSlug(form.customProviderName);
    if (!slug) {
      return "Provider name must include letters or numbers";
    }
  }

  if (form.provider === "custom" && !form.baseUrl?.trim()) {
    return "Base URL is required for custom providers";
  }

  if (!form.apiKey.trim() && !form.apiKeyConfigured) {
    return "API Key is required";
  }

  return null;
}

export function thirdPartyModelToSaveRequest(
  form: ThirdPartyModelForm,
  existingModelId?: string | null,
): ThirdPartyModelSaveRequest {
  const provider =
    form.provider === "custom"
      ? normalizeCustomProviderSlug(form.customProviderName)
      : form.provider;
  return {
    provider,
    model: form.model.trim(),
    baseUrl: form.baseUrl,
    region: form.awsRegion,
    apiKey: form.apiKey,
    apiKeyEnv: form.apiKeyEnv,
    awsSecretAccessKey: form.awsSecretAccessKey,
    awsSessionToken: form.awsSessionToken,
    existingModelId: existingModelId ?? undefined,
  };
}

export function testThirdPartyModelConnectivity(
  form: ThirdPartyModelForm,
): Promise<ThirdPartyModelConnectivityResult> {
  return invokeCommand<ThirdPartyModelConnectivityResult>("models_test_third_party", {
    request: thirdPartyModelToSaveRequest(form),
  });
}

export function getThirdPartyModelEditForm(
  modelId: string,
): Promise<ThirdPartyModelEditDto> {
  return invokeCommand<ThirdPartyModelEditDto>("models_third_party_edit_form", { modelId });
}

export function thirdPartyEditDtoToForm(dto: ThirdPartyModelEditDto): ThirdPartyModelForm {
  const slug = dto.provider.trim().toLowerCase();
  const isKnown = KNOWN_THIRD_PARTY_PROVIDERS.has(slug as ThirdPartyProvider);
  const provider = (isKnown ? slug : "custom") as ThirdPartyProvider;
  const meta = THIRD_PARTY_PROVIDERS.find((entry) => entry.value === provider);

  return {
    provider,
    customProviderName: isKnown ? "" : dto.provider,
    model: dto.model,
    baseUrl: dto.baseUrl,
    apiKey: "",
    apiKeyEnv: dto.apiKeyEnv ?? meta?.apiKeyEnv ?? null,
    awsSecretAccessKey: "",
    awsSessionToken: "",
    awsRegion: dto.region,
    apiKeyConfigured: dto.apiKeyConfigured,
    awsSecretAccessKeyConfigured: dto.awsSecretAccessKeyConfigured,
    awsSessionTokenConfigured: dto.awsSessionTokenConfigured,
  };
}

export async function loadThirdPartyModelForm(
  modelId: string,
): Promise<ThirdPartyModelForm> {
  const dto = await getThirdPartyModelEditForm(modelId);
  return thirdPartyEditDtoToForm(dto);
}

export async function saveThirdPartyModelForm(
  form: ThirdPartyModelForm,
  existingModelId?: string | null,
): Promise<ThirdPartyModelForm> {
  await saveThirdPartyModel(thirdPartyModelToSaveRequest(form, existingModelId));
  return {
    ...form,
    apiKey: "",
    awsSecretAccessKey: "",
    awsSessionToken: "",
    apiKeyConfigured: form.apiKeyConfigured || form.apiKey.trim().length > 0,
    awsSecretAccessKeyConfigured:
      form.awsSecretAccessKeyConfigured || form.awsSecretAccessKey.trim().length > 0,
    awsSessionTokenConfigured:
      form.awsSessionTokenConfigured || form.awsSessionToken.trim().length > 0,
  };
}
