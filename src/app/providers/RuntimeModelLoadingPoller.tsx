import { useAppStore } from "@/app/store/AppStore";
import { useRuntimeModelLoadingPoll } from "@/shared/hooks/useRuntimeModelLoading";

/** Keeps runtime model-loading state fresh across all routes. */
export function RuntimeModelLoadingPoller() {
  const { backendConnected } = useAppStore();
  useRuntimeModelLoadingPoll(backendConnected);
  return null;
}
