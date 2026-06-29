import { useEffect, useState } from "react";

export const PAGE_SIZE_OPTIONS = [5, 10, 25, 50, 100] as const;
export type PageSize = (typeof PAGE_SIZE_OPTIONS)[number];

const PREFIX = "promptlab:page-size:";

function loadPageSize(key: string, fallback: PageSize): PageSize {
  if (typeof window === "undefined") return fallback;
  try {
    const value = Number(window.localStorage.getItem(`${PREFIX}${key}`));
    return PAGE_SIZE_OPTIONS.includes(value as PageSize) ? (value as PageSize) : fallback;
  } catch {
    return fallback;
  }
}

function savePageSize(key: string, size: PageSize): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(`${PREFIX}${key}`, String(size));
  } catch {
    // Ignore quota errors.
  }
}

export function usePageSizePreference(key: string, defaultSize: PageSize = 5) {
  const [pageSize, setPageSize] = useState<PageSize>(() => loadPageSize(key, defaultSize));

  useEffect(() => {
    savePageSize(key, pageSize);
  }, [key, pageSize]);

  return [pageSize, setPageSize] as const;
}
