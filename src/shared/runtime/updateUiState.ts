/** Survives Settings tab switches (AboutPanel remount). */

export type UpdateUiState = {
  busy: "check" | "install" | null;
  error: string | null;
  status: string | null;
  latestVersion: string | null;
  updateAvailable: boolean;
  downloadPct: number | null;
};

const INITIAL: UpdateUiState = {
  busy: null,
  error: null,
  status: null,
  latestVersion: null,
  updateAvailable: false,
  downloadPct: null,
};

let state: UpdateUiState = { ...INITIAL };
const listeners = new Set<() => void>();

export function getUpdateUiState(): UpdateUiState {
  return state;
}

export function patchUpdateUiState(patch: Partial<UpdateUiState>): void {
  state = { ...state, ...patch };
  for (const listener of listeners) listener();
}

export function subscribeUpdateUiState(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
