import { invokeCommand } from "./invoke";
import { withModelOperationTimeout } from "./modelOperationTimeout";

export type ModelCapabilitiesDto = {
  chat: boolean;
  completion: boolean;
  embeddings: boolean;
};

export type ModelEntryDto = {
  id: string;
  name: string;
  provider: string;
  version: string;
  format: string;
  sizeBytes: number | null;
  sizeGb: number;
  verified: boolean;
  path: string;
  sha256: string | null;
  capabilities: ModelCapabilitiesDto;
  status: string;
};

export type ModelCatalogEntryDto = {
  id: string;
  name: string;
  provider: string;
  version: string;
  description: string;
  purpose: string;
  recommended: boolean;
  sizeBytes: number | null;
  sizeGb: number | null;
  quant: string | null;
  capabilities: ModelCapabilitiesDto;
  engine: string;
  format: string;
  downloadUrl: string | null;
  sha256: string | null;
  sizeLabel: string | null;
};

export type ModelRegistryInfoDto = {
  entryCount: number;
  remoteMerged: boolean;
  remoteUrl: string | null;
  sourcePath: string | null;
  totalModels: number;
  validModels: number;
  invalidModels: number;
};

export type RegistryValidationIssueDto = {
  id: string;
  field: string;
  message: string;
};

export type ModelRegistryDiagnosticsDto = {
  totalModels: number;
  validModels: number;
  invalidModels: number;
  validIds: string[];
  invalidIds: string[];
  issues: RegistryValidationIssueDto[];
  healthy: boolean;
};

export type ModelInstallRequest = {
  catalogId: string;
};

export type ModelImportRequest = {
  name: string;
  path: string;
};

export type ModelDownloadRequest = {
  catalogId: string;
};

export type ModelDownloadProgressDto = {
  catalogId: string;
  status: string;
  downloadedBytes: number;
  totalBytes: number | null;
  remainingBytes: number | null;
  percent: number | null;
  speedBytesPerSec: number | null;
  etaSeconds: number | null;
  resumed: boolean;
  destination: string;
  error: string | null;
};

export type ModelVaultStatsDto = {
  vaultPath: string;
  registeredCount: number;
  installedLocalCount: number;
  installedBytes: number;
  installedGb: number;
};

export type ModelDownloadStatusDto = {
  active: boolean;
  progress: ModelDownloadProgressDto | null;
  installed: ModelEntryDto | null;
};

export type ModelInferenceTestResult = {
  ok: boolean;
  mode: string;
  sample: string;
  message: string;
};

export function listModels(): Promise<ModelEntryDto[]> {
  return invokeCommand<ModelEntryDto[]>("models_list");
}

export function getModelsRegistryInfo(): Promise<ModelRegistryInfoDto> {
  return invokeCommand<ModelRegistryInfoDto>("models_registry_info");
}

export function getModelsRegistryDiagnostics(): Promise<ModelRegistryDiagnosticsDto> {
  return invokeCommand<ModelRegistryDiagnosticsDto>("models_registry_diagnostics");
}

export function browseModels(): Promise<ModelCatalogEntryDto[]> {
  return invokeCommand<ModelCatalogEntryDto[]>("models_browse");
}

export function installModel(request: ModelInstallRequest): Promise<ModelEntryDto> {
  return invokeCommand<ModelEntryDto>("models_install", { request });
}

export function importModelGguf(request: ModelImportRequest): Promise<ModelEntryDto> {
  return invokeCommand<ModelEntryDto>("models_import_gguf", { request });
}

export function importModelZip(request: ModelImportRequest): Promise<ModelEntryDto> {
  return invokeCommand<ModelEntryDto>("models_import_zip", { request });
}

export type ThirdPartyModelSaveRequest = {
  provider: string;
  model: string;
  baseUrl?: string | null;
  region?: string | null;
  apiKey?: string;
  apiKeyEnv?: string | null;
  awsSecretAccessKey?: string;
  awsSessionToken?: string;
  existingModelId?: string | null;
  /** Apply a successful Test Connection as Verified on save. */
  markVerified?: boolean;
  testLatencyMs?: number | null;
};

export function saveThirdPartyModel(
  request: ThirdPartyModelSaveRequest,
): Promise<ModelEntryDto> {
  return invokeCommand<ModelEntryDto>("models_save_third_party", { request });
}

export function startModelDownload(request: ModelDownloadRequest): Promise<ModelDownloadProgressDto> {
  return invokeCommand<ModelDownloadProgressDto>("models_download_start", { request });
}

export function getModelDownloadStatus(): Promise<ModelDownloadStatusDto> {
  return invokeCommand<ModelDownloadStatusDto>("models_download_status");
}

export function pauseModelDownload(): Promise<ModelDownloadProgressDto> {
  return invokeCommand<ModelDownloadProgressDto>("models_download_pause");
}

export function resumeModelDownload(): Promise<ModelDownloadProgressDto> {
  return invokeCommand<ModelDownloadProgressDto>("models_download_resume");
}

export function cancelModelDownload(): Promise<void> {
  return invokeCommand<void>("models_download_cancel");
}

export function retryModelDownloadVerify(
  request: ModelDownloadRequest,
): Promise<ModelDownloadStatusDto> {
  return invokeCommand<ModelDownloadStatusDto>("models_download_retry_verify", { request });
}

export function cancelModelDownloadVerify(): Promise<ModelDownloadProgressDto> {
  return invokeCommand<ModelDownloadProgressDto>("models_download_cancel_verify");
}

export function removeModel(modelId: string): Promise<ModelEntryDto> {
  return invokeCommand<ModelEntryDto>("models_remove", { modelId });
}

export function verifyModel(modelId: string): Promise<{
  filePath: string;
  expectedSha256: string | null;
  actualSha256: string;
  sizeBytes: number;
  valid: boolean;
}> {
  return invokeCommand("models_verify", { modelId });
}

export type ModelConnectionTestResult = {
  ok: boolean;
  provider: string;
  model: string;
  latencyMs: number;
  message: string;
  sampleResponse?: string | null;
};

export function testModelConnection(
  modelId: string,
): Promise<ModelConnectionTestResult> {
  return withModelOperationTimeout(
    invokeCommand<ModelConnectionTestResult>("models_test_connection", { modelId }),
    "Connection test",
  );
}

export function testModelInference(modelId: string): Promise<ModelInferenceTestResult> {
  return withModelOperationTimeout(
    invokeCommand<ModelInferenceTestResult>("models_test_inference", { modelId }),
    "Model verify",
  );
}

export function testModelEmbeddings(
  modelId: string,
  input?: string,
): Promise<ModelInferenceTestResult> {
  return invokeCommand<ModelInferenceTestResult>("models_test_embeddings", {
    modelId,
    input: input ?? null,
  });
}

export function getModelsVaultPath(): Promise<string> {
  return invokeCommand<string>("models_vault_path");
}

export function getModelsVaultStats(): Promise<ModelVaultStatsDto> {
  return invokeCommand<ModelVaultStatsDto>("models_vault_stats");
}
