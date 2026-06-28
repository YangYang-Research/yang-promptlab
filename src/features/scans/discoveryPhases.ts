export type DiscoveryPhaseId =
  | "discovering_endpoints"
  | "fingerprinting"
  | "inferring_schemas"
  | "detecting_capabilities"
  | "classifying_endpoints"
  | "calculating_risk"
  | "saving_metadata";

export type DiscoveryPhase = {
  id: DiscoveryPhaseId;
  label: string;
};

export const DISCOVERY_PHASES: DiscoveryPhase[] = [
  { id: "discovering_endpoints", label: "Discovering Endpoints" },
  { id: "fingerprinting", label: "Fingerprinting" },
  { id: "inferring_schemas", label: "Inferring Schemas" },
  { id: "detecting_capabilities", label: "Detecting Capabilities" },
  { id: "classifying_endpoints", label: "Classifying Endpoints" },
  { id: "calculating_risk", label: "Calculating Risk" },
  { id: "saving_metadata", label: "Saving Metadata" },
];

export type PhaseStatus = "pending" | "active" | "complete";

export function phaseIndexFromId(phaseId: string | null | undefined): number {
  if (!phaseId) return 0;
  const idx = DISCOVERY_PHASES.findIndex((p) => p.id === phaseId);
  return idx >= 0 ? idx : 0;
}

export function phaseStatuses(
  running: boolean,
  completed: boolean,
  activeIndex: number,
): PhaseStatus[] {
  if (completed) {
    return DISCOVERY_PHASES.map(() => "complete" as const);
  }
  if (!running) {
    return DISCOVERY_PHASES.map(() => "pending" as const);
  }
  return DISCOVERY_PHASES.map((_, index) => {
    if (index < activeIndex) return "complete";
    if (index === activeIndex) return "active";
    return "pending";
  });
}

export function endpointSourceLabel(kind: string, sourceUrl: string | null): "Manual" | "Discovery" | "Plugin" {
  if (kind === "manual" || sourceUrl === "manual") {
    return "Manual";
  }
  if (sourceUrl?.includes("plugin") || kind === "plugin") {
    return "Plugin";
  }
  return "Discovery";
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
