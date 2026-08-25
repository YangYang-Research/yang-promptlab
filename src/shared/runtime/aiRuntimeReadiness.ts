import { getRuntimeConfiguration, type RuntimeConfigurationDto } from "@/shared/ipc/runtime";

export type AiRuntimeReadiness = {
  ready: boolean;
  configuration: RuntimeConfigurationDto | null;
  message: string;
};

export async function checkAiRuntimeReadiness(): Promise<AiRuntimeReadiness> {
  try {
    const configuration = await getRuntimeConfiguration();
    const settings = configuration.settings;

    if (!settings.selectedModelId) {
      return {
        ready: false,
        configuration,
        message: "Select a remote AI model in AI Runtime before creating a project.",
      };
    }

    if (
      configuration.mode === "local" ||
      configuration.mode === "not_configured"
    ) {
      // Legacy configs should already be migrated; treat as not ready until remote.
      return {
        ready: false,
        configuration,
        message:
          "AI Runtime is remote-only. Add a remote provider model in Models, then select it in AI Runtime.",
      };
    }

    const selected = settings.thirdPartyModels.find(
      (model) => model.id === settings.selectedModelId,
    );
    if (selected && !selected.configured) {
      return {
        ready: false,
        configuration,
        message:
          "The selected remote model is missing API credentials. Configure it in Models first.",
      };
    }

    return {
      ready: true,
      configuration,
      message: "",
    };
  } catch {
    return {
      ready: false,
      configuration: null,
      message: "Could not verify AI Runtime. Open AI Runtime and confirm your model is ready.",
    };
  }
}

export async function assertAiRuntimeReady(
  backendConnected: boolean,
): Promise<AiRuntimeReadiness> {
  if (!backendConnected) {
    return {
      ready: false,
      configuration: null,
      message:
        "PromptLab backend is not connected. Run the desktop app to configure AI Runtime before creating a project.",
    };
  }
  return checkAiRuntimeReadiness();
}
