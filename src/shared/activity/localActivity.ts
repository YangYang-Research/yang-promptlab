import type { ActivityItem } from "@/shared/types";
import {
  listRecentActivity,
  recordRecentActivity,
  replaceRecentActivity,
  type ActivityItemDto,
} from "@/shared/ipc/activity";

const LEGACY_STORAGE_KEY = "promptlab:local-activity";
export const LOCAL_ACTIVITY_CHANGED_EVENT = "promptlab:local-activity";

export type LocalActivityType = "runtime" | "model";

export type LocalActivityInput = {
  type: LocalActivityType;
  message: string;
  /** Stable-ish id suffix; defaults to timestamp. */
  id?: string;
};

let cache: ActivityItem[] = [];
let hydrated = false;

function asActivityItem(dto: ActivityItemDto): ActivityItem {
  return {
    id: dto.id,
    type: dto.type,
    message: dto.message,
    timestamp: dto.timestamp,
  };
}

function readLegacyLocalStorage(): ActivityItem[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(LEGACY_STORAGE_KEY);
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

function clearLegacyLocalStorage() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(LEGACY_STORAGE_KEY);
  } catch {
    // ignore
  }
}

function notifyChanged() {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(LOCAL_ACTIVITY_CHANGED_EVENT));
  }
}

/** Load from SQLite; one-shot migrate legacy localStorage into DB. */
export async function hydrateLocalActivity(): Promise<ActivityItem[]> {
  try {
    let items = (await listRecentActivity()).map(asActivityItem);
    const legacy = readLegacyLocalStorage();
    if (items.length === 0 && legacy.length > 0) {
      items = (
        await replaceRecentActivity(
          legacy.map((item) => ({
            id: item.id,
            type: item.type as LocalActivityType,
            message: item.message,
            timestamp: item.timestamp,
          })),
        )
      ).map(asActivityItem);
    }
    if (legacy.length > 0) {
      clearLegacyLocalStorage();
    }
    cache = items;
    hydrated = true;
    notifyChanged();
    return cache;
  } catch {
    // Mock / disconnected UI: fall back to legacy localStorage only.
    cache = readLegacyLocalStorage();
    hydrated = true;
    return cache;
  }
}

export function listLocalActivity(): ActivityItem[] {
  return cache;
}

export async function recordLocalActivity(
  input: LocalActivityInput,
): Promise<ActivityItem> {
  if (!hydrated) {
    await hydrateLocalActivity();
  }
  try {
    const dto = await recordRecentActivity({
      type: input.type,
      message: input.message,
      id: input.id,
    });
    const item = asActivityItem(dto);
    cache = [item, ...cache.filter((existing) => existing.id !== item.id)].slice(0, 50);
    notifyChanged();
    return item;
  } catch {
    // Offline / mock: keep optimistic local-only cache.
    const timestamp = new Date().toISOString();
    const item: ActivityItem = {
      id: input.id ?? `local-${input.type}-${timestamp}`,
      type: input.type,
      message: input.message,
      timestamp,
    };
    cache = [item, ...cache.filter((existing) => existing.id !== item.id)].slice(0, 50);
    notifyChanged();
    return item;
  }
}
