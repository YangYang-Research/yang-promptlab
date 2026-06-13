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
  sizeBytes: number | null;
  sizeGb: number | null;
  quant: string | null;
  capabilities: ModelCapabilitiesDto;
  ollamaTag: string | null;
};

export type ModelInstallRequest = {
  catalogId: string;
  ollamaBaseUrl?: string | null;
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

export function browseModels(): Promise<ModelCatalogEntryDto[]> {
  return invokeCommand<ModelCatalogEntryDto[]>("models_browse");
}

export function installModel(request: ModelInstallRequest): Promise<ModelEntryDto> {
  return invokeCommand<ModelEntryDto>("models_install", { request });
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
