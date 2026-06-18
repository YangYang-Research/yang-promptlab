export type DiscoveryPhaseId = "crawl" | "js" | "api" | "graphql" | "openapi" | "fingerprint";

export type DiscoveryPhase = {
  id: DiscoveryPhaseId;
  label: string;
};

export const DISCOVERY_PHASES: DiscoveryPhase[] = [
  { id: "crawl", label: "Crawl" },
  { id: "js", label: "JavaScript" },
  { id: "api", label: "API" },
  { id: "graphql", label: "GraphQL" },
  { id: "openapi", label: "OpenAPI" },
  { id: "fingerprint", label: "Fingerprint" },
];

export type PhaseStatus = "pending" | "active" | "complete";

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

export function endpointSourceLabel(kind: string, sourceUrl: string | null): "Manual" | "Discovery" {
  if (kind === "manual" || sourceUrl === "manual") {
    return "Manual";
  }
  return "Discovery";
}
