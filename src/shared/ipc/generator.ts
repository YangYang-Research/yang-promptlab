import { invokeCommand } from "./invoke";

export type GeneratorMode = "static_pack" | "template_mutation" | "local_llm";

export type GeneratorGenerateRequest = {
  profileId: string;
  categories: string[];
  disabledTests: string[];
  mode: GeneratorMode;
};

export type PromptPayloadDto = {
  id: string;
  name: string;
  category: string;
  content: string;
};

export type GeneratorStatsDto = {
  categoryCount: number;
  sourceCount: number;
  payloadCount: number;
  variantCount: number;
};

export type PromptPayloadsDto = {
  mode: GeneratorMode;
  payloads: PromptPayloadDto[];
  payloadIds: string[];
  stats: GeneratorStatsDto;
  summary: string;
  llmNote: string | null;
};

export function generatePromptPayloads(
  request: GeneratorGenerateRequest,
): Promise<PromptPayloadsDto> {
  return invokeCommand<PromptPayloadsDto>("generator_generate", { request });
}
