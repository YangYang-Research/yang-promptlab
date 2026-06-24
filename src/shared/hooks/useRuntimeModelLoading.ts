import { useCallback, useEffect, useSyncExternalStore } from "react";

import { getRuntimeConfiguration } from "@/shared/ipc/runtime";

const POLL_MS = 500;

type RuntimeModelLoadingSnapshot = {
  modelLoading: boolean;
  loadingModelId: string | null;
};

let snapshot: RuntimeModelLoadingSnapshot = {
  modelLoading: false,
  loadingModelId: null,
};

let pollTimer: ReturnType<typeof setInterval> | null = null;
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) {
    listener();
  }
}

function deriveSnapshot(
  config: Awaited<ReturnType<typeof getRuntimeConfiguration>>,
): RuntimeModelLoadingSnapshot {
  const isLoading = config.modelLoadInProgress === true;
  return {
    modelLoading: isLoading,
    loadingModelId: isLoading ? (config.settings.selectedModelId ?? null) : null,
  };
}

export async function refreshRuntimeModelLoading(): Promise<RuntimeModelLoadingSnapshot> {
  const config = await getRuntimeConfiguration();
  const next = deriveSnapshot(config);
  const changed =
    next.modelLoading !== snapshot.modelLoading ||
    next.loadingModelId !== snapshot.loadingModelId;
  snapshot = next;
  if (changed) {
    emit();
  }
  return snapshot;
}

function startPolling() {
  if (pollTimer !== null) {
    return;
  }
  void refreshRuntimeModelLoading().catch(() => undefined);
  pollTimer = window.setInterval(() => {
    void refreshRuntimeModelLoading().catch(() => undefined);
  }, POLL_MS);
}

function stopPolling() {
  if (pollTimer !== null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot() {
  return snapshot;
}

/** App-wide poll — survives route changes. */
export function useRuntimeModelLoadingPoll(enabled: boolean) {
  useEffect(() => {
    if (!enabled) {
      stopPolling();
      snapshot = { modelLoading: false, loadingModelId: null };
      emit();
      return;
    }
    startPolling();
    return () => {
      stopPolling();
    };
  }, [enabled]);
}

/**
 * Read global runtime model loading state (backed by app-wide poll).
 */
export function useRuntimeModelLoading(_enabled = true) {
  const state = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const refresh = useCallback(async () => refreshRuntimeModelLoading(), []);
  return { ...state, refresh };
}
