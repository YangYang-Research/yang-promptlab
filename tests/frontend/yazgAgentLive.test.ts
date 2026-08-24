import { describe, expect, it } from "vitest";

import { isYazgAgentLive } from "@/shared/runtime/yazgAgentLive";
import type { RuntimeConfigurationDto } from "@/shared/ipc/runtime";

function baseConfig(
  overrides: Partial<RuntimeConfigurationDto> = {},
): RuntimeConfigurationDto {
  return {
    mode: "third_party",
    provider: "openai",
    modelName: "gpt-4o-mini",
    runtimeName: "cloud",
    runtimeVersion: null,
    statusLabel: "Live",
    connectivity: "Connected",
    lastHealthCheck: null,
    settings: {
      initialized: true,
      selectedModelId: "model-1",
      selectedModelName: "gpt-4o-mini",
      thirdPartyModels: [],
    },
    runtimeStatus: {
      modelLoaded: false,
      requiresAttention: false,
      message: "",
      loadedModelPath: null,
      runtimeVersion: null,
    },
    ...overrides,
  } as RuntimeConfigurationDto;
}

describe("isYazgAgentLive", () => {
  it("is false when not configured", () => {
    expect(isYazgAgentLive(null)).toBe(false);
    expect(isYazgAgentLive(baseConfig({ mode: "not_configured" }))).toBe(false);
  });

  it("is true for third-party with model and live connectivity", () => {
    expect(isYazgAgentLive(baseConfig())).toBe(true);
  });

  it("is false for local without loaded model", () => {
    expect(
      isYazgAgentLive(
        baseConfig({
          mode: "local",
          runtimeStatus: {
            modelLoaded: false,
            requiresAttention: false,
            message: "",
            loadedModelPath: null,
            runtimeVersion: null,
          } as RuntimeConfigurationDto["runtimeStatus"],
        }),
      ),
    ).toBe(false);
  });
});
