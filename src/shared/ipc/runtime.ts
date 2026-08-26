import { invokeCommand } from "./invoke";

export type RuntimeLifecycleState =
  | "not_installed"
  | "downloading"
  | "installing"
  | "installed"
  | "starting"
  | "running"
  | "busy"
  | "stopping"
  | "stopped"
  | "updating"
  | "failed";

export type RuntimeStatusDto = {
  lifecycleState: RuntimeLifecycleState;
  runtimeVersion: string | null;
  backend: string | null;
  platform: string | null;
  installPath: string | null;
  installed: boolean;
  verified: boolean;
  binaryAvailable: boolean;
  baseUrl: string;
  modelLoaded: boolean;
  loadedModelPath: string | null;
  message: string;
  requiresAttention: boolean;
  lastError: string | null;
  recommendedRuntime: string | null;
};

export type RuntimeHardwareDto = {
  os: string;
  arch: string;
  cpu: string;
  cpuCores: number;
  ramBytes: number;
  gpuVendor: string | null;
  gpuName: string | null;
  vramBytes: number | null;
  cuda: boolean;
  metal: boolean;
  vulkan: boolean;
  avx2: boolean;
  diskFreeBytes?: number | null;
  detectedAt: string;
};

export type RuntimeHealthReport = {
  lifecycleState: string;
  processAlive: boolean;
  endpointReachable: boolean;
  latencyMs: number;
  memoryBytes: number | null;
  gpuMemoryBytes: number | null;
  modelLoaded: boolean;
  message: string;
};

export type RuntimeLogEntry = {
  timestamp: string;
  level: string;
  message: string;
};

export type AiInferenceRoute = "third_party";

export type AiInferenceModelOptionDto = {
  id: string;
  name: string;
  provider: string;
  verified: boolean;
  configured: boolean;
  statusLabel: string;
};

export type AiInferenceSettingsDto = {
  route: AiInferenceRoute;
  initialized: boolean;
  selectedModelId: string | null;
  selectedModelName: string | null;
  thirdPartyAvailable: boolean;
  thirdPartyModels: AiInferenceModelOptionDto[];
  message: string;
  connectivityTestOk?: boolean | null;
  connectivityTestDetail?: string | null;
};

export type RuntimeInferenceRouteRequest = {
  route: AiInferenceRoute;
  selectedModelId?: string | null;
};

export type RuntimeConfigurationDto = {
  mode: "not_configured" | "third_party" | "local";
  statusLabel: string;
  provider: string | null;
  modelName: string | null;
  runtimeName: string | null;
  runtimeVersion: string | null;
  connectivity: string | null;
  lastHealthCheck: string | null;
  modelLoadInProgress: boolean;
  modelTestInProgress: boolean;
  settings: AiInferenceSettingsDto;
  runtimeStatus: RuntimeStatusDto;
};

export function getRuntimeConfiguration(): Promise<RuntimeConfigurationDto> {
  return invokeCommand<RuntimeConfigurationDto>("runtime_configuration");
}

export function getRuntimeInferenceSettings(): Promise<AiInferenceSettingsDto> {
  return invokeCommand<AiInferenceSettingsDto>("runtime_inference_settings");
}

export function setRuntimeInferenceRoute(
  request: RuntimeInferenceRouteRequest,
): Promise<AiInferenceSettingsDto> {
  return invokeCommand<AiInferenceSettingsDto>("runtime_set_inference_route", { request });
}

export function resetRuntimeConfig(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_delete");
}

export type RuntimeTrafficBucket = {
  atMs: number;
  sent: number;
  received: number;
};

export type RuntimeTrafficSnapshot = {
  windowMs: number;
  bucketMs: number;
  buckets: RuntimeTrafficBucket[];
  totalSent: number;
  totalReceived: number;
  continuous: boolean;
};

export function getRuntimeTrafficStats(
  windowMs = 60_000,
  bucketMs = 1_000,
): Promise<RuntimeTrafficSnapshot> {
  return invokeCommand<RuntimeTrafficSnapshot>("runtime_traffic_stats", {
    windowMs,
    bucketMs,
  });
}

export type AgentTokenUsageRow = {
  agentId: string;
  label: string;
  note?: string | null;
  inputTokens: number;
  outputTokens: number;
  calls: number;
};

export type TokenUsageSnapshot = {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCalls: number;
  agents: AgentTokenUsageRow[];
};

export function getRuntimeTokenUsage(): Promise<TokenUsageSnapshot> {
  return invokeCommand<TokenUsageSnapshot>("runtime_token_usage");
}

export function resetRuntimeTokenUsage(): Promise<TokenUsageSnapshot> {
  return invokeCommand<TokenUsageSnapshot>("runtime_token_usage_reset");
}

export type RuntimeConnectivityResult = {
  ok: boolean;
  provider: string;
  model: string;
  latencyMs: number;
  message: string;
  sampleResponse?: string | null;
};

export function testRuntimeConnectivity(): Promise<RuntimeConnectivityResult> {
  return invokeCommand<RuntimeConnectivityResult>("runtime_test_connectivity");
}

export function testRuntimeInference(): Promise<RuntimeConnectivityResult> {
  return invokeCommand<RuntimeConnectivityResult>("runtime_test_inference");
}

export type JudgeRoleWeightsDto = {
  judge: number;
  classifier: number;
  attacker: number;
  defaultLlm: number;
  updatedAt: string;
};

export type UpdateJudgeRoleWeightsRequest = {
  judge: number;
  classifier: number;
  attacker: number;
  defaultLlm: number;
};

export const DEFAULT_JUDGE_ROLE_WEIGHTS: Omit<JudgeRoleWeightsDto, "updatedAt"> = {
  judge: 0.85,
  classifier: 0.8,
  attacker: 0.75,
  defaultLlm: 0.65,
};

export function getJudgeRoleWeights(): Promise<JudgeRoleWeightsDto> {
  return invokeCommand<JudgeRoleWeightsDto>("runtime_judge_role_weights");
}

export function setJudgeRoleWeights(
  request: UpdateJudgeRoleWeightsRequest,
): Promise<JudgeRoleWeightsDto> {
  return invokeCommand<JudgeRoleWeightsDto>("runtime_set_judge_role_weights", { request });
}
