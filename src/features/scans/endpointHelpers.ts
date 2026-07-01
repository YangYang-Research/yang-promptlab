export function endpointSourceLabel(
  kind: string,
  sourceUrl: string | null,
): "Manual" | "Auto" | "Plugin" {
  if (kind === "manual" || sourceUrl === "manual") {
    return "Manual";
  }
  if (sourceUrl?.includes("plugin") || kind === "plugin") {
    return "Plugin";
  }
  return "Auto";
}

export type EndpointFilterState = {
  endpointType: string;
  framework: string;
  capability: string;
  auth: string;
  minRisk: number;
  minConfidence: number;
  source: string;
};

export const DEFAULT_ENDPOINT_FILTERS: EndpointFilterState = {
  endpointType: "",
  framework: "",
  capability: "",
  auth: "",
  minRisk: 0,
  minConfidence: 0,
  source: "",
};

export function matchesEndpointFilters(
  endpoint: {
    endpoint_type: string;
    ai_framework: string | null;
    risk_score: number;
    metadata_confidence: number;
    discovery_source: string;
    auth_required: boolean;
    metadata: {
      capabilities: {
        supportsChat: boolean;
        supportsStreaming: boolean;
        supportsEmbedding: boolean;
        supportsTools: boolean;
        supportsAgent: boolean;
        supportsVision: boolean;
      };
    } | null;
  },
  filters: EndpointFilterState,
): boolean {
  if (filters.endpointType && endpoint.endpoint_type !== filters.endpointType) return false;
  if (filters.framework) {
    const fw =
      endpoint.ai_framework ??
      (endpoint.metadata as { fingerprint?: { framework?: string } } | null)?.fingerprint
        ?.framework ??
      "";
    if (fw !== filters.framework) return false;
  }
  if (filters.source && endpoint.discovery_source !== filters.source) return false;
  if (endpoint.risk_score < filters.minRisk) return false;
  if (endpoint.metadata_confidence < filters.minConfidence / 100) return false;
  if (filters.auth === "required" && !endpoint.auth_required) return false;
  if (filters.auth === "anonymous" && endpoint.auth_required) return false;

  const caps = endpoint.metadata?.capabilities;
  if (filters.capability && caps) {
    const ok = matchCapabilityFilter(filters.capability, caps);
    if (!ok) return false;
  } else if (filters.capability) {
    return false;
  }

  return true;
}

function matchCapabilityFilter(
  capability: string,
  caps: NonNullable<EndpointFilterState extends never ? never : {
    supportsChat: boolean;
    supportsStreaming: boolean;
    supportsEmbedding: boolean;
    supportsTools: boolean;
    supportsAgent: boolean;
    supportsVision: boolean;
  }>,
): boolean {
  switch (capability) {
    case "chat":
      return caps.supportsChat;
    case "streaming":
      return caps.supportsStreaming;
    case "embedding":
      return caps.supportsEmbedding;
    case "tools":
      return caps.supportsTools;
    case "agent":
      return caps.supportsAgent;
    case "vision":
      return caps.supportsVision;
    default:
      return true;
  }
}
