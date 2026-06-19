import { useEffect, useState } from "react";

export type ViewMode = "table" | "list";

const PREFIX = "aisec:view:";

function loadViewMode(key: string, fallback: ViewMode): ViewMode {
  if (typeof window === "undefined") return fallback;
  try {
    const value = window.localStorage.getItem(`${PREFIX}${key}`);
    return value === "list" || value === "table" ? value : fallback;
  } catch {
    return fallback;
  }
}

function saveViewMode(key: string, mode: ViewMode): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(`${PREFIX}${key}`, mode);
  } catch {
    // Ignore quota errors.
  }
}

export function useViewPreference(key: string, defaultMode: ViewMode = "table") {
  const [mode, setMode] = useState<ViewMode>(() => loadViewMode(key, defaultMode));

  useEffect(() => {
    saveViewMode(key, mode);
  }, [key, mode]);

  return [mode, setMode] as const;
}
