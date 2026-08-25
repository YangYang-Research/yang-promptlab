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

export type ModelVaultStatsDto = {
  vaultPath: string;
  registeredCount: number;
  installedLocalCount: number;
  installedBytes: number;
  installedGb: number;
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
