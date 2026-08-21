const STORAGE_KEY = "promptlab:setup-complete";

export function isSetupComplete(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function markSetupComplete(): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, "1");
  } catch {
    // Ignore quota / private-mode errors.
  }
}

export function clearSetupComplete(): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Ignore.
  }
}

export const SETUP_COMPLETE_STORAGE_KEY = STORAGE_KEY;
