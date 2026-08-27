export type GlobalSearchKind =
  | "project"
  | "target"
  | "scan"
  | "finding"
  | "report"
  | "technique";

export type GlobalSearchHit = {
  id: string;
  kind: GlobalSearchKind;
  title: string;
  subtitle: string;
  to: string;
};

export const GLOBAL_SEARCH_KIND_LABEL: Record<GlobalSearchKind, string> = {
  project: "Projects",
  target: "Targets",
  scan: "Scans",
  finding: "Findings",
  report: "Reports",
  technique: "Attack Techniques",
};

export const GLOBAL_SEARCH_KIND_ORDER: GlobalSearchKind[] = [
  "project",
  "target",
  "scan",
  "finding",
  "report",
  "technique",
];

export type GlobalSearchGroup = {
  kind: GlobalSearchKind;
  label: string;
  hits: GlobalSearchHit[];
};

export function groupSearchHits(hits: GlobalSearchHit[]): GlobalSearchGroup[] {
  const buckets = new Map<GlobalSearchKind, GlobalSearchHit[]>();
  for (const kind of GLOBAL_SEARCH_KIND_ORDER) buckets.set(kind, []);
  for (const hit of hits) {
    buckets.get(hit.kind)?.push(hit);
  }
  return GLOBAL_SEARCH_KIND_ORDER.flatMap((kind) => {
    const groupHits = buckets.get(kind) ?? [];
    if (groupHits.length === 0) return [];
    return [{ kind, label: GLOBAL_SEARCH_KIND_LABEL[kind], hits: groupHits }];
  });
}

const KINDS = new Set<string>(Object.keys(GLOBAL_SEARCH_KIND_LABEL));
const ROUTE_PREFIXES = [
  "/projects/",
  "/targets/",
  "/scans/",
  "/findings/",
  "/reports/",
  "/attack-categories/",
];

export function asGlobalSearchHit(hit: {
  id: string;
  kind: string;
  title: string;
  subtitle: string;
  to: string;
}): GlobalSearchHit | null {
  if (!KINDS.has(hit.kind)) return null;
  if (!ROUTE_PREFIXES.some((prefix) => hit.to.startsWith(prefix))) return null;
  return {
    id: hit.id,
    kind: hit.kind as GlobalSearchKind,
    title: hit.title,
    subtitle: hit.subtitle,
    to: hit.to,
  };
}
