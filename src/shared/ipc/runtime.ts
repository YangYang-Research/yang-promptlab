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

export function getRuntimeStatus(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_status");
}

export function installRuntime(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_install");
}

export function startRuntime(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_start");
}

export function stopRuntime(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_stop");
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
