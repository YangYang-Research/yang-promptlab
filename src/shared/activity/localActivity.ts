import type { ActivityItem } from "@/shared/types";

const STORAGE_KEY = "promptlab:local-activity";
const MAX_ITEMS = 50;
export const LOCAL_ACTIVITY_CHANGED_EVENT = "promptlab:local-activity";

export type LocalActivityType = "runtime" | "model";

export type LocalActivityInput = {
  type: LocalActivityType;
  message: string;
  /** Stable-ish id suffix; defaults to timestamp. */
  id?: string;
};

function readAll(): ActivityItem[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as ActivityItem[];
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (item) =>
        item &&
        typeof item.id === "string" &&
        typeof item.message === "string" &&
        typeof item.timestamp === "string" &&
        (item.type === "runtime" || item.type === "model"),
    );
  } catch {
    return [];
  }
}

function writeAll(items: ActivityItem[]): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(items.slice(0, MAX_ITEMS)));
  } catch {
    // Ignore quota / private-mode errors.
  }
}

export function listLocalActivity(): ActivityItem[] {
  return readAll();
}

export function recordLocalActivity(input: LocalActivityInput): ActivityItem {
  const timestamp = new Date().toISOString();
  const item: ActivityItem = {
    id: input.id ?? `local-${input.type}-${timestamp}`,
    type: input.type,
    message: input.message,
    timestamp,
  };
  const next = [item, ...readAll().filter((existing) => existing.id !== item.id)].slice(
    0,
    MAX_ITEMS,
  );
  writeAll(next);
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(LOCAL_ACTIVITY_CHANGED_EVENT));
  }
  return item;
}
