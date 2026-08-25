import { connectivityStatusVariant } from "@/shared/components";
import {
  getRuntimeConfiguration,
  type RuntimeConfigurationDto,
} from "@/shared/ipc/runtime";

export const YAZG_AGENT_OFFLINE_MESSAGE =
  "Yazg Agent is offline. Open AI Runtime, configure a model, and wait until Yazg shows Live before continuing.";

/** Yazg Agent is Live only when a model is active and connectivity succeeded. */
export function isYazgAgentLive(
  configuration: RuntimeConfigurationDto | null | undefined,
): boolean {
  if (!configuration || configuration.mode === "local") {
    return false;
  }

  const hasModel = Boolean(
    configuration.modelName ?? configuration.settings.selectedModelId,
  );
  if (!hasModel) return false;

  return (
    connectivityStatusVariant(configuration.connectivity) === "success" ||
    configuration.statusLabel === "Running" ||
    configuration.statusLabel === "Live"
  );
}

export type YazgAgentLiveCheck = {
  live: boolean;
  configuration: RuntimeConfigurationDto | null;
  message: string;
};

/** Fetch runtime config and report whether Yazg Agent is Live. */
export async function checkYazgAgentLive(): Promise<YazgAgentLiveCheck> {
  try {
    const configuration = await getRuntimeConfiguration();
    if (isYazgAgentLive(configuration)) {
      return { live: true, configuration, message: "" };
    }
    return {
      live: false,
      configuration,
      message: YAZG_AGENT_OFFLINE_MESSAGE,
    };
  } catch {
    return {
      live: false,
      configuration: null,
      message: YAZG_AGENT_OFFLINE_MESSAGE,
    };
  }
}

/**
 * Require Yazg Agent Live before an agent-backed action.
 * Returns the check result; callers should abort when `live` is false.
 */
export async function assertYazgAgentLive(
  backendConnected = true,
): Promise<YazgAgentLiveCheck> {
  if (!backendConnected) {
    return {
      live: false,
      configuration: null,
      message:
        "PromptLab backend is not connected. Run the desktop app so Yazg Agent can reach AI Runtime.",
    };
  }
  return checkYazgAgentLive();
}
