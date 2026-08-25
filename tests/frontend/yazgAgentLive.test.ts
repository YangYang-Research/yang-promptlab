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
  it("is false when missing config or model", () => {
    expect(isYazgAgentLive(null)).toBe(false);
    expect(
      isYazgAgentLive(
        baseConfig({
          modelName: null,
          settings: {
            ...baseConfig().settings,
            selectedModelId: null,
            selectedModelName: null,
          },
        }),
      ),
    ).toBe(false);
  });

  it("is true for remote with model and live connectivity", () => {
    expect(isYazgAgentLive(baseConfig())).toBe(true);
  });

  it("is false for legacy local mode", () => {
    expect(
      isYazgAgentLive(
        baseConfig({
          mode: "local",
        }),
      ),
    ).toBe(false);
  });
});
