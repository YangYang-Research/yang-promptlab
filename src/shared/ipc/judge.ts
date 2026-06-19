import { invokeCommand } from "./invoke";

export type JudgeMode = "deterministic" | "local_llm" | "remote_llm" | "consensus";

export type LocalProvider = "ollama" | "llama_cpp";

export type RemoteProvider = "openai" | "anthropic" | "gemini" | "openrouter";

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
    hint: "OpenAI, Anthropic, Gemini, OpenRouter",
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
  { value: "gemini", label: "Gemini" },
  { value: "openrouter", label: "OpenRouter" },
];

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
  remoteModel: "gpt-4o-mini",
  remoteApiKey: "",
  remoteApiKeyEnv: "OPENAI_API_KEY",
  consensusThreshold: 0.55,
  minConfidence: 0.45,
  llmMaxTokens: 512,
  llmTemperature: 0.1,
  categories: [],
};
