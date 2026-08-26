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
    modelLoadInProgress: false,
    modelTestInProgress: false,
    settings: {
      route: "third_party",
      initialized: true,
      selectedModelId: "model-1",
      selectedModelName: "gpt-4o-mini",
      thirdPartyAvailable: true,
      thirdPartyModels: [],
      message: "",
    },
    runtimeStatus: {
      lifecycleState: "running",
      runtimeVersion: null,
      backend: "remote",
      platform: null,
      installPath: null,
      installed: true,
      verified: true,
      baseUrl: "",
      message: "",
      requiresAttention: false,
      lastError: null,
      recommendedRuntime: null,
    },
    ...overrides,
  };
}

describe("isYazgAgentLive", () => {
  it("is false when missing config or model", () => {
    expect(isYazgAgentLive(null)).toBe(false);
    expect(
      isYazgAgentLive(
        baseConfig({
          modelName: null,
          settings: {
            route: "third_party",
            initialized: true,
            selectedModelId: null,
            selectedModelName: null,
            thirdPartyAvailable: false,
            thirdPartyModels: [],
            message: "",
          },
        }),
      ),
    ).toBe(false);
  });

  it("is true when third-party connectivity is live", () => {
    expect(isYazgAgentLive(baseConfig())).toBe(true);
  });

  it("is false for not_configured mode", () => {
    expect(isYazgAgentLive(baseConfig({ mode: "not_configured" }))).toBe(false);
  });
});
