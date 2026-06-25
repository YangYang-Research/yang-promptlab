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

export type RuntimeInstallProgressEvent = {
  step: string;
  message: string;
  phase: number;
};

export const RUNTIME_INSTALL_PROGRESS_EVENT = "runtime-install-progress";

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

export type RuntimeBenchmarkResult = {
  ok: boolean;
  latencyMs: number;
  tokensPerSec: number;
  tokensPredicted: number;
  memoryBytes: number | null;
  gpuMemoryBytes: number | null;
  message: string;
  measuredAt: string;
};

export type RuntimeLogEntry = {
  timestamp: string;
  level: string;
  message: string;
};

export type AiInferenceRoute = "third_party" | "local";

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
  localAvailable: boolean;
  thirdPartyModels: AiInferenceModelOptionDto[];
  localModels: AiInferenceModelOptionDto[];
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

export function getRuntimeStatus(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_status");
}

export function reinitializeRuntimeEngine(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_repair");
}

/** @deprecated Use {@link reinitializeRuntimeEngine} — embedded libllama has no separate install step. */
export function installRuntime(): Promise<RuntimeStatusDto> {
  return reinitializeRuntimeEngine();
}

/** @deprecated Use {@link reinitializeRuntimeEngine}. */
export function repairRuntime(): Promise<RuntimeStatusDto> {
  return reinitializeRuntimeEngine();
}

export function resetRuntimeConfig(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_delete");
}

/** @deprecated Use {@link resetRuntimeConfig}. */
export function deleteRuntime(): Promise<RuntimeStatusDto> {
  return resetRuntimeConfig();
}

export function startRuntime(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_start");
}

export function stopRuntime(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_stop");
}

export function loadRuntimeModel(modelId: string): Promise<RuntimeConfigurationDto> {
  return invokeCommand<RuntimeConfigurationDto>("runtime_load_model", {
    request: { modelId },
  });
}

export function unloadRuntimeModel(): Promise<RuntimeConfigurationDto> {
  return invokeCommand<RuntimeConfigurationDto>("runtime_unload_model");
}

export function restartRuntime(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_restart");
}

export function getRuntimeHealth(): Promise<RuntimeHealthReport> {
  return invokeCommand<RuntimeHealthReport>("runtime_health");
}

export function runRuntimeBenchmark(): Promise<RuntimeBenchmarkResult> {
  return invokeCommand<RuntimeBenchmarkResult>("runtime_benchmark");
}

export function getRuntimeLogs(limit = 100): Promise<RuntimeLogEntry[]> {
  return invokeCommand<RuntimeLogEntry[]>("runtime_logs", { limit });
}

export function refreshRuntimeHardware(): Promise<RuntimeHardwareDto> {
  return invokeCommand<RuntimeHardwareDto>("hardware_refresh");
}

export function getRuntimeHardware(): Promise<RuntimeHardwareDto | null> {
  return invokeCommand<RuntimeHardwareDto | null>("runtime_hardware");
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
