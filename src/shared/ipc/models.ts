import { invokeCommand } from "./invoke";

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
  ollamaTag: string | null;
};

export type ModelRegistryInfoDto = {
  entryCount: number;
  remoteMerged: boolean;
  remoteUrl: string | null;
  sourcePath: string | null;
};

export type ModelInstallRequest = {
  catalogId: string;
  ollamaBaseUrl?: string | null;
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
  percent: number | null;
  resumed: boolean;
  destination: string;
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

export function testModelInference(modelId: string): Promise<ModelInferenceTestResult> {
  return invokeCommand<ModelInferenceTestResult>("models_test_inference", { modelId });
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
