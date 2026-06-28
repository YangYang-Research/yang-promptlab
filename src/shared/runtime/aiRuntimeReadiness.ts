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

    if (!settings.initialized || configuration.mode === "not_configured") {
      return {
        ready: false,
        configuration,
        message:
          "AI Runtime is not configured. Choose a third-party API model or load a local model before creating a project.",
      };
    }

    if (!settings.selectedModelId) {
      return {
        ready: false,
        configuration,
        message: "Select an AI model in AI Runtime before creating a project.",
      };
    }

    if (configuration.mode === "third_party") {
      const selected = settings.thirdPartyModels.find(
        (model) => model.id === settings.selectedModelId,
      );
      if (selected && !selected.configured) {
        return {
          ready: false,
          configuration,
          message:
            "The selected third-party model is missing API credentials. Configure it in AI Runtime first.",
        };
      }
    }

    if (configuration.mode === "local") {
      if (!configuration.runtimeStatus.modelLoaded) {
        return {
          ready: false,
          configuration,
          message: "Load a local model in AI Runtime before creating a project.",
        };
      }
      if (configuration.runtimeStatus.requiresAttention) {
        return {
          ready: false,
          configuration,
          message:
            configuration.runtimeStatus.message ||
            "Local AI Runtime needs attention. Open AI Runtime and resolve the issue first.",
        };
      }
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
        "AISec backend is not connected. Run the desktop app to configure AI Runtime before creating a project.",
    };
  }
  return checkAiRuntimeReadiness();
}
