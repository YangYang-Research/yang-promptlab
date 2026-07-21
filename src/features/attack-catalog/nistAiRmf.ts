/**
 * NIST AI RMF mapping for Attack Factory techniques.
 * Derived from category + OWASP tags (no separate DB column).
 */

export type NistFunctionId = "govern" | "map" | "measure" | "manage";

export type NistBrowseId = "all" | NistFunctionId;

export type NistRef = {
  /** Short badge label, e.g. `MAP 1.5`. */
  label: string;
  functionId: NistFunctionId;
};

export const NIST_BROWSE: { id: NistBrowseId; label: string }[] = [
  { id: "all", label: "All" },
  { id: "govern", label: "Govern" },
  { id: "map", label: "Map" },
  { id: "measure", label: "Measure" },
  { id: "manage", label: "Manage" },
];

type TechniqueLike = {
  categoryId: string;
  owasp: string | null;
};

function owaspTags(owasp: string | null): string[] {
  if (!owasp) return [];
  return owasp
    .split(",")
    .map((part) => part.trim().toUpperCase())
    .filter(Boolean);
}

function pushUnique(refs: NistRef[], next: NistRef) {
  if (refs.some((ref) => ref.label === next.label)) return;
  refs.push(next);
}

/**
 * Map a technique to NIST AI RMF control badges used in Attack Factory.
 * Prefer OWASP family signals; fall back to attack category.
 */
export function nistRefsForTechnique(row: TechniqueLike): NistRef[] {
  const refs: NistRef[] = [];
  const tags = owaspTags(row.owasp);
  const category = row.categoryId.toLowerCase();

  const has = (prefix: string) => tags.some((tag) => tag.startsWith(prefix));

  if (has("LLM01") || category.includes("prompt_injection")) {
    pushUnique(refs, { label: "MAP 1.5", functionId: "map" });
    pushUnique(refs, { label: "MEASURE 2.3", functionId: "measure" });
  }
  if (has("LLM02") || category.includes("jailbreak")) {
    pushUnique(refs, { label: "MAP 1.5", functionId: "map" });
    pushUnique(refs, { label: "MEASURE 2.2", functionId: "measure" });
  }
  if (has("LLM07") || category.includes("system_prompt")) {
    pushUnique(refs, { label: "MAP 1.1", functionId: "map" });
    pushUnique(refs, { label: "MEASURE 2.3", functionId: "measure" });
  }
  if (has("LLM06") || category.includes("rag") || category.includes("cross_user")) {
    pushUnique(refs, { label: "MAP 2.3", functionId: "map" });
    pushUnique(refs, { label: "MEASURE 2.3", functionId: "measure" });
  }
  if (has("LLM08") || category.includes("tool_abuse") || category.includes("agent_goal")) {
    pushUnique(refs, { label: "GOVERN 1.5", functionId: "govern" });
    pushUnique(refs, { label: "MANAGE 4.1", functionId: "manage" });
  }
  if (has("LLM04") || category.includes("memory")) {
    pushUnique(refs, { label: "MAP 1.5", functionId: "map" });
    pushUnique(refs, { label: "MANAGE 2.4", functionId: "manage" });
  }
  if (has("LLM05") || has("LLM10") || category.includes("encoding")) {
    pushUnique(refs, { label: "MEASURE 2.2", functionId: "measure" });
    pushUnique(refs, { label: "MANAGE 1.3", functionId: "manage" });
  }
  if (has("ASI")) {
    pushUnique(refs, { label: "GOVERN 1.5", functionId: "govern" });
    pushUnique(refs, { label: "MANAGE 1.3", functionId: "manage" });
  }
  if (has("MCP") || category.includes("mcp")) {
    pushUnique(refs, { label: "GOVERN 1.2", functionId: "govern" });
    pushUnique(refs, { label: "MANAGE 4.1", functionId: "manage" });
  }

  if (refs.length === 0) {
    pushUnique(refs, { label: "MAP 1.1", functionId: "map" });
    pushUnique(refs, { label: "MEASURE 1.1", functionId: "measure" });
  }

  return refs;
}

export function nistFunctionsForTechnique(row: TechniqueLike): NistFunctionId[] {
  return [
    ...new Set(nistRefsForTechnique(row).map((ref) => ref.functionId)),
  ];
}

export function matchesNistBrowse(row: TechniqueLike, browse: NistBrowseId): boolean {
  if (browse === "all") return true;
  return nistFunctionsForTechnique(row).includes(browse);
}
