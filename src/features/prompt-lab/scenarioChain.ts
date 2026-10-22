/** Seed → reply attack tree for Prompt Lab Scenario (local history). */

export type ScenarioChainNode = {
  id: string;
  /** null = root seed from Plan. Follow-up creates a child of the active node. */
  parentId: string | null;
  seed: string;
  /** Target reply after this seed was sent; null until follow-up / fire. */
  reply: string | null;
};

export type ScenarioHistoryEntry = {
  id: string;
  createdAt: string;
  scenario: string;
  goal: string;
  nodes: ScenarioChainNode[];
};

const STORAGE_KEY = "promptlab.scenarioHistory.v1";
const MAX_ENTRIES = 40;

export function newChainNodeId(): string {
  return `n_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export function newHistoryId(): string {
  return `h_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

/** Normalize legacy linear chains (no parentId) into a linked tree. */
export function normalizeNodes(raw: unknown[]): ScenarioChainNode[] {
  const out: ScenarioChainNode[] = [];
  for (let i = 0; i < raw.length; i++) {
    const item = raw[i];
    if (!item || typeof item !== "object") continue;
    const row = item as Partial<ScenarioChainNode>;
    if (typeof row.id !== "string" || typeof row.seed !== "string") continue;
    const parentId =
      row.parentId === undefined
        ? i === 0
          ? null
          : out[out.length - 1]?.id ?? null
        : row.parentId;
    out.push({
      id: row.id,
      parentId,
      seed: row.seed,
      reply: typeof row.reply === "string" ? row.reply : null,
    });
  }
  return out;
}

export function childrenOf(
  nodes: ScenarioChainNode[],
  parentId: string | null,
): ScenarioChainNode[] {
  return nodes.filter((n) => n.parentId === parentId);
}

export function nodeDepth(nodes: ScenarioChainNode[], id: string): number {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  let depth = 0;
  let cur = byId.get(id);
  const guard = new Set<string>();
  while (cur?.parentId) {
    if (guard.has(cur.id)) break;
    guard.add(cur.id);
    depth += 1;
    cur = byId.get(cur.parentId);
  }
  return depth;
}

/** Columns left→right by depth; siblings stack in the same column. */
export function layoutColumns(nodes: ScenarioChainNode[]): ScenarioChainNode[][] {
  if (nodes.length === 0) return [];
  const maxDepth = Math.max(...nodes.map((n) => nodeDepth(nodes, n.id)), 0);
  const cols: ScenarioChainNode[][] = Array.from({ length: maxDepth + 1 }, () => []);
  for (const n of nodes) {
    cols[nodeDepth(nodes, n.id)].push(n);
  }
  return cols;
}

/** Every completed seed→reply, oldest first, plus the turn being followed up. */
export function priorTurnsForFollowup(
  nodes: ScenarioChainNode[],
  lastPrompt: string,
  lastResponse: string,
  extra: { seed: string; reply: string }[] = [],
): string {
  const turns: { seed: string; reply: string }[] = [];
  const push = (seed: string, reply: string) => {
    const s = seed.trim();
    const r = reply.trim();
    if (!s || !r) return;
    if (turns.some((t) => t.seed === s && t.reply === r)) return;
    turns.push({ seed: s, reply: r });
  };
  for (const turn of extra) push(turn.seed, turn.reply);
  for (const node of nodes) push(node.seed, node.reply ?? "");
  push(lastPrompt, lastResponse);
  return turns
    .map(
      (turn, index) =>
        `Turn ${index + 1}\nSeed: ${turn.seed}\nReply: ${turn.reply}`,
    )
    .join("\n\n");
}

export function loadScenarioHistory(): ScenarioHistoryEntry[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map(coerceHistoryEntry)
      .filter((e): e is ScenarioHistoryEntry => e !== null)
      .slice(0, MAX_ENTRIES);
  } catch {
    return [];
  }
}

export function saveScenarioHistory(entries: ScenarioHistoryEntry[]): void {
  try {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(entries.slice(0, MAX_ENTRIES)),
    );
  } catch {
    /* quota / private mode */
  }
}

export function upsertScenarioHistory(
  entry: ScenarioHistoryEntry,
  existing: ScenarioHistoryEntry[],
): ScenarioHistoryEntry[] {
  const without = existing.filter((e) => e.id !== entry.id);
  return [entry, ...without].slice(0, MAX_ENTRIES);
}

export function deleteScenarioHistory(
  id: string,
  existing: ScenarioHistoryEntry[],
): ScenarioHistoryEntry[] {
  return existing.filter((e) => e.id !== id);
}

export function clearScenarioHistory(): void {
  saveScenarioHistory([]);
}

function coerceHistoryEntry(value: unknown): ScenarioHistoryEntry | null {
  if (!value || typeof value !== "object") return null;
  const row = value as ScenarioHistoryEntry;
  if (
    typeof row.id !== "string" ||
    typeof row.createdAt !== "string" ||
    typeof row.scenario !== "string" ||
    typeof row.goal !== "string" ||
    !Array.isArray(row.nodes)
  ) {
    return null;
  }
  return {
    ...row,
    nodes: normalizeNodes(row.nodes as unknown[]),
  };
}

export function previewText(text: string, max = 72): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length > max ? `${oneLine.slice(0, max)}…` : oneLine;
}
