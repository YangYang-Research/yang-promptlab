import { invokeCommand } from "./invoke";

export type MutatorSettingsDto = {
  enabledMutators: string[];
  /** category_id → ordered mutator ids */
  categoryMutators: Record<string, string[]>;
  updatedAt: string;
};

export type UpdateMutatorSettingsRequest = {
  enabledMutators: string[];
  categoryMutators: Record<string, string[]>;
};

export function getMutatorSettings(): Promise<MutatorSettingsDto> {
  return invokeCommand<MutatorSettingsDto>("mutator_settings_get");
}

export function setMutatorSettings(
  request: UpdateMutatorSettingsRequest,
): Promise<MutatorSettingsDto> {
  return invokeCommand<MutatorSettingsDto>("mutator_settings_set", { request });
}

export type PromptLabVariantDto = {
  mutatorId: string;
  content: string;
};

export type PromptLabUpgradeDto = {
  content: string;
  canary?: string | null;
};

export type PromptLabExpandDto = {
  variants: PromptLabVariantDto[];
};

/** Attack Factory GeneratePromptAgent — upgrade seed via AI Runtime. */
export function upgradePromptLab(prompt: string): Promise<PromptLabUpgradeDto> {
  return invokeCommand<PromptLabUpgradeDto>("prompt_lab_upgrade", { prompt });
}

/** Expand all mutators on seed as-is (LLM kinds via GeneratePromptAgent). */
export function expandPromptLab(prompt: string): Promise<PromptLabExpandDto> {
  return invokeCommand<PromptLabExpandDto>("prompt_lab_expand", { prompt });
}

export type PromptLabHistoryDto = {
  id: string;
  seedPrompt: string;
  variants: PromptLabVariantDto[];
  createdAt: string;
};

export function listPromptLabHistory(limit = 50): Promise<PromptLabHistoryDto[]> {
  return invokeCommand<PromptLabHistoryDto[]>("prompt_lab_history_list", { limit });
}

export function getPromptLabHistory(id: string): Promise<PromptLabHistoryDto> {
  return invokeCommand<PromptLabHistoryDto>("prompt_lab_history_get", { id });
}

export function deletePromptLabHistory(id: string): Promise<void> {
  return invokeCommand<void>("prompt_lab_history_delete", { id });
}
