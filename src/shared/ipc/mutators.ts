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
