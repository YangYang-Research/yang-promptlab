import type { EndpointDto } from "@/shared/ipc/client";
import type { AttackCategoryId } from "./attackProfiles";

const PLATFORM_LABELS: Record<string, string> = {
  openwebui: "OpenWebUI",
  dify: "Dify",
  flowise: "Flowise",
  langflow: "Langflow",
  librechat: "LibreChat",
  mcp_server: "MCP Server",
  openai_api: "OpenAI API",
  anthropic_api: "Anthropic API",
  azure_openai_api: "Azure OpenAI",
  ollama_api: "Ollama",
  ollama: "Ollama",
  openai: "OpenAI",
  gemini_api: "Google Gemini",
  bedrock_api: "AWS Bedrock",
};

const ENDPOINT_TYPE_LABELS: Record<string, string> = {
  ai_chat: "AI Chat",
  ai_agent: "AI Agent",
  embedding: "Embedding",
  completion: "Completion",
  image_generation: "Image Generation",
  speech: "Speech",
  moderation: "Moderation",
  workflow: "Workflow",
  tool_endpoint: "Tool Endpoint",
  mcp: "MCP",
  unknown_ai: "Unknown AI",
  non_ai: "Non-AI",
};

const FINGERPRINT_TO_ATTACK: Record<string, AttackCategoryId> = {
  prompt_injection: "prompt_injection",
  jailbreak: "jailbreak",
  system_prompt_leakage: "system_prompt_extraction",
  rag_leakage: "rag_leakage",
  tool_abuse: "tool_abuse",
  mcp_abuse: "mcp_abuse",
  memory_poisoning: "memory_poisoning",
  agent_goal_hijacking: "agent_goal_hijacking",
  cross_user_leakage: "cross_user_leakage",
};

export function platformLabel(platform: string): string {
  if (!platform) return "Unknown";
  return PLATFORM_LABELS[platform] ?? platform.replace(/_/g, " ");
}

export function endpointTypeLabel(endpointType: string | null | undefined): string {
  if (!endpointType) return "Unknown";
  return ENDPOINT_TYPE_LABELS[endpointType] ?? endpointType.replace(/_/g, " ");
}

export function mapFingerprintCategory(category: string): AttackCategoryId | null {
  return FINGERPRINT_TO_ATTACK[category] ?? null;
}

export function aggregateAttackSuggestions(
  endpoints: EndpointDto[],
  selectedIds: string[],
): { categories: AttackCategoryId[]; reasons: Map<AttackCategoryId, string> } {
  const selected = new Set(selectedIds);
  const reasons = new Map<AttackCategoryId, string>();
  const categories = new Set<AttackCategoryId>();

  for (const endpoint of endpoints) {
    if (!selected.has(endpoint.id)) continue;
    for (const rec of endpoint.attack_recommendations ?? []) {
      const mapped = mapFingerprintCategory(rec.category);
      if (!mapped) continue;
      categories.add(mapped);
      if (!reasons.has(mapped)) {
        reasons.set(mapped, rec.reason);
      }
    }
  }

  return {
    categories: [...categories],
    reasons,
  };
}

export function aggregatePlatformSummary(
  endpoints: EndpointDto[],
  selectedIds: string[],
): {
  platform: string;
  framework: string;
  memoryEnabled: boolean;
  toolsEnabled: boolean;
  ragEnabled: boolean;
}[] {
  const selected = new Set(selectedIds);
  const seen = new Set<string>();
  const profiles: {
    platform: string;
    framework: string;
    memoryEnabled: boolean;
    toolsEnabled: boolean;
    ragEnabled: boolean;
  }[] = [];

  for (const endpoint of endpoints) {
    if (!selected.has(endpoint.id)) continue;
    const framework =
      endpoint.metadata?.fingerprint.framework ??
      endpoint.ai_framework ??
      endpoint.metadata?.classification.aiFramework ??
      "";
    if (!framework || seen.has(framework)) continue;
    seen.add(framework);
    const caps = endpoint.metadata?.capabilities;
    profiles.push({
      platform: framework,
      framework,
      memoryEnabled: caps?.supportsMemory ?? false,
      toolsEnabled: caps?.supportsTools ?? false,
      ragEnabled: caps?.supportsAgent ?? false,
    });
  }

  return profiles;
}

export function endpointPlatformLabel(endpoint: EndpointDto): string {
  const framework = endpoint.metadata?.fingerprint.framework ?? endpoint.ai_framework;
  if (framework) return platformLabel(framework);
  const provider = endpoint.metadata?.fingerprint.provider;
  if (provider) return platformLabel(`${provider}_api`);
  return "—";
}

export function endpointHasAiMetadata(endpoint: EndpointDto): boolean {
  return Boolean(endpoint.metadata) && endpoint.endpoint_type !== "non_ai";
}
