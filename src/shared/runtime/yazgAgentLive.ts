import { connectivityStatusVariant } from "@/shared/components";
import type { RuntimeConfigurationDto } from "@/shared/ipc/runtime";

/** Yazg Agent is Live only when a model is active and connectivity succeeded. */
export function isYazgAgentLive(
  configuration: RuntimeConfigurationDto | null | undefined,
): boolean {
  if (!configuration || configuration.mode === "not_configured") {
    return false;
  }

  if (configuration.mode === "third_party") {
    const hasModel = Boolean(
      configuration.modelName ?? configuration.settings.selectedModelId,
    );
    if (!hasModel) return false;
  } else if (configuration.mode === "local") {
    if (!configuration.runtimeStatus.modelLoaded) return false;
  }

  return (
    connectivityStatusVariant(configuration.connectivity) === "success" ||
    configuration.statusLabel === "Running" ||
    configuration.statusLabel === "Live"
  );
}
