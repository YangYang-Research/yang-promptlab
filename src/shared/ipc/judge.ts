import { invokeCommand } from "./invoke";

export type JudgeMode = "deterministic" | "local_llm" | "remote_llm" | "consensus";

export type LocalProvider = "ollama" | "llama_cpp";

export type RemoteProvider =
  | "openai"
  | "anthropic"
  | "gemini"
  | "openrouter"
  | "azure"
  | "bedrock";

export type JudgeConfigDto = {
  mode: JudgeMode;
  localProvider: LocalProvider;
  localBaseUrl: string;
  localModel: string;
  localModelPath: string | null;
  localVaultModelId: string | null;
  localLlamaBinary: string;
  localLlamaPort: number;
  remoteProvider: RemoteProvider;
  remoteBaseUrl: string | null;
  remoteModel: string;
  remoteApiKey: string;
  remoteApiKeyEnv: string | null;
  remoteApiKeyConfigured?: boolean;
  remoteAwsSecretAccessKey: string;
  remoteAwsSecretAccessKeyConfigured?: boolean;
  remoteAwsRegion: string | null;
  remoteAwsSessionToken: string;
  remoteAwsSessionTokenConfigured?: boolean;
  consensusThreshold: number;
  minConfidence: number;
  llmMaxTokens: number;
  llmTemperature: number;
  categories: string[];
};

export type JudgeConnectivityResult = {
  ok: boolean;
  provider: string;
  model: string;
  latencyMs: number;
  message: string;
  sampleResponse?: string | null;
};

export function getJudgeConfig(): Promise<JudgeConfigDto> {
  return invokeCommand<JudgeConfigDto>("judge_config_get");
}

export function saveJudgeConfig(config: JudgeConfigDto): Promise<JudgeConfigDto> {
  return invokeCommand<JudgeConfigDto>("judge_config_save", { config });
}

export function testJudgeConnectivity(
  config?: JudgeConfigDto,
): Promise<JudgeConnectivityResult> {
  return invokeCommand<JudgeConnectivityResult>("judge_test_connectivity", {
    config: config ?? null,
  });
}

export function testJudgeModel(config?: JudgeConfigDto): Promise<JudgeConnectivityResult> {
  return invokeCommand<JudgeConnectivityResult>("judge_test_model", {
    config: config ?? null,
  });
}

export const JUDGE_MODES: Array<{ value: JudgeMode; label: string; hint: string }> = [
  {
    value: "deterministic",
    label: "Deterministic",
    hint: "Rule + regex scoring only (offline, fast)",
  },
  {
    value: "local_llm",
    label: "Local LLM",
    hint: "Ollama or llama.cpp GGUF models",
  },
  {
    value: "remote_llm",
    label: "Remote LLM",
    hint: "Uses third-party models configured on the Models page",
  },
  {
    value: "consensus",
    label: "Consensus",
    hint: "Deterministic + LLM combined verdict",
  },
];

export const LOCAL_PROVIDERS: Array<{ value: LocalProvider; label: string }> = [
  { value: "ollama", label: "Ollama" },
  { value: "llama_cpp", label: "llama.cpp (GGUF)" },
];

export const REMOTE_PROVIDERS: Array<{ value: RemoteProvider; label: string }> = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "gemini", label: "Google" },
  { value: "azure", label: "Azure" },
  { value: "bedrock", label: "AWS Bedrock" },
  { value: "openrouter", label: "OpenRouter" },
];

export type ThirdPartyProvider = Extract<
  RemoteProvider,
  "openai" | "anthropic" | "gemini" | "azure" | "bedrock"
>;

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
];

export function validateThirdPartyConfig(config: JudgeConfigDto): string | null {
  if (!config.remoteModel.trim()) {
    return "Model name is required";
  }

  if (config.remoteProvider === "bedrock") {
    if (!config.remoteApiKey.trim() && !config.remoteApiKeyConfigured) {
      return "Access Key ID is required";
    }
    if (!config.remoteAwsSecretAccessKey.trim() && !config.remoteAwsSecretAccessKeyConfigured) {
      return "Secret Access Key is required";
    }
    if (!config.remoteAwsRegion?.trim()) {
      return "Region is required";
    }
    const accessKey = config.remoteApiKey.trim();
    if (
      accessKey.startsWith("ASIA") &&
      !config.remoteAwsSessionToken.trim() &&
      !config.remoteAwsSessionTokenConfigured
    ) {
      return "Session Token is required for temporary AWS credentials (ASIA access keys)";
    }
    return null;
  }

  if (config.remoteProvider === "azure" && !config.remoteBaseUrl?.trim()) {
    return "Endpoint URL is required";
  }

  if (!config.remoteApiKey.trim() && !config.remoteApiKeyConfigured) {
    return "API Key is required";
  }

  return null;
}

export function thirdPartyTestConfig(config: JudgeConfigDto): JudgeConfigDto {
  return { ...config, mode: "remote_llm" };
}

export const DEFAULT_JUDGE_CONFIG: JudgeConfigDto = {
  mode: "deterministic",
  localProvider: "ollama",
  localBaseUrl: "http://127.0.0.1:11434",
  localModel: "llama3",
  localModelPath: null,
  localVaultModelId: null,
  localLlamaBinary: "llama-server",
  localLlamaPort: 8081,
  remoteProvider: "openai",
  remoteBaseUrl: null,
  remoteModel: "",
  remoteApiKey: "",
  remoteApiKeyEnv: "OPENAI_API_KEY",
  remoteAwsSecretAccessKey: "",
  remoteAwsRegion: null,
  remoteAwsSessionToken: "",
  consensusThreshold: 0.55,
  minConfidence: 0.45,
  llmMaxTokens: 512,
  llmTemperature: 0.1,
  categories: [],
};
