import type { EndpointDto } from "@/shared/ipc/client";
import type { AttackCategoryId } from "./attackProfiles";

export type PlatformProfile = {
  platform: string;
  version: string;
  authType: string;
  llmProvider: string;
  memoryEnabled: boolean;
  toolsEnabled: boolean;
  ragEnabled: boolean;
};

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
  gemini_api: "Google Gemini",
  bedrock_api: "AWS Bedrock",
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
    for (const rec of endpoint.fingerprint?.attackRecommendations ?? []) {
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
): PlatformProfile[] {
  const selected = new Set(selectedIds);
  const seen = new Set<string>();
  const profiles: PlatformProfile[] = [];

  for (const endpoint of endpoints) {
    if (!selected.has(endpoint.id)) continue;
    const profile = endpoint.fingerprint?.platformProfile;
    if (!profile?.platform || seen.has(profile.platform)) continue;
    seen.add(profile.platform);
    profiles.push(profile);
  }

  return profiles;
}

export function endpointPlatformLabel(endpoint: EndpointDto): string {
  const profile = endpoint.fingerprint?.platformProfile;
  if (profile?.platform) {
    return platformLabel(profile.platform);
  }
  if (endpoint.fingerprint?.primaryProvider) {
    return platformLabel(`${endpoint.fingerprint.primaryProvider}_api`);
  }
  return "—";
}
