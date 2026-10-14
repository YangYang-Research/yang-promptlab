/** Survives Prompt Lab route remounts while upgrade / expand IPC still runs. */

import type { PromptLabVariantDto } from "@/shared/ipc/mutators";

export type PromptLabUiState = {
  prompt: string;
  busy: "upgrade" | "expand" | null;
  error: string | null;
  variants: PromptLabVariantDto[];
};

const INITIAL: PromptLabUiState = {
  prompt: "",
  busy: null,
  error: null,
  variants: [],
};

let state: PromptLabUiState = { ...INITIAL };
const listeners = new Set<() => void>();

export function getPromptLabUiState(): PromptLabUiState {
  return state;
}

export function patchPromptLabUiState(patch: Partial<PromptLabUiState>): void {
  state = { ...state, ...patch };
  for (const listener of listeners) listener();
}

export function subscribePromptLabUiState(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
