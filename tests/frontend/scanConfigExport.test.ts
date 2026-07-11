import { afterEach, describe, expect, it } from "vitest";

import {
  SCAN_CONFIG_FORMAT,
  SCAN_CONFIG_VERSION,
  buildScanConfigExport,
  clearScanConfigImport,
  parseScanConfigExport,
  scanConfigToWizardPatch,
  stashScanConfigImport,
  createSessionFromScanConfigImport,
} from "@/features/scans/scanConfigExport";
import { createInitialTargetProfile } from "@/features/scans/targetProfile";

const sampleProfile = {
  ...createInitialTargetProfile(),
  provider: "openai_compatible" as const,
  framework: "openai_compatible",
  method: "POST",
  baseUrl: "https://api.example.com",
  path: "/v1/chat/completions",
  headersJson: JSON.stringify({
    "Content-Type": "application/json",
    Authorization: "Bearer sk-local-test",
  }),
  requestTemplate: JSON.stringify({
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: "{{PROMPT}}" }],
  }),
};

const samplePayloadStrategy = {
  strategy: "mutation",
  mutationLevel: "medium",
  variantsPerTest: 5,
  maxTotalPayloads: 20,
  enableContextAwareness: false,
  enableConversationMemory: false,
  enableResponseAdaptation: false,
  enablePayloadDeduplication: true,
  enableCrossCategoryMutation: false,
};

const samplePlaybook = {
  profile: "standard",
  categories: ["prompt_injection", "jailbreak"],
  disabled_tests: ["prompt_injection.basic"],
  agent_mode: true,
  max_agent_attempts: 3,
  reflection_enabled: true,
  adaptive_planning: true,
  payload_strategy: samplePayloadStrategy,
};

const richWizardPlan = {
  profileId: "deep",
  recommendedProfileId: "standard",
  suggestedCategories: ["prompt_injection", "jailbreak", "system_prompt_extraction"],
  profileModes: [
    {
      profileId: "deep",
      categories: ["prompt_injection", "jailbreak", "system_prompt_extraction"],
      executionStrategy: "agentic",
      maxAttempts: 5,
      reflectionEnabled: true,
      adaptivePlanning: true,
      payloadStrategy: samplePayloadStrategy,
      disabledTests: [],
    },
  ],
  categories: ["prompt_injection", "jailbreak", "system_prompt_extraction"],
  disabledTests: ["pi-direct-override"],
  capabilityGraph: ["tools", "memory"],
  attackGraph: [
    {
      category: "prompt_injection",
      priority: 1,
      risk: 90,
      confidence: 0.9,
      dependencies: [],
      enabled: true,
    },
    {
      category: "jailbreak",
      priority: 2,
      risk: 80,
      confidence: 0.85,
      dependencies: ["prompt_injection"],
      enabled: true,
    },
    {
      category: "system_prompt_extraction",
      priority: 3,
      risk: 70,
      confidence: 0.8,
      dependencies: [],
      enabled: true,
    },
  ],
  executionStrategy: "agentic",
  maxAttempts: 5,
  reflectionEnabled: true,
  adaptivePlanning: true,
  rationales: [
    {
      category: "prompt_injection",
      reason: "Open chat completions surface",
      priority: 1,
      source: "ai_runtime",
    },
  ],
  confidence: 0.91,
  summary: "Deep plan for example API",
  riskScore: 78,
  riskLevel: "high",
  estimatedRequests: 120,
  estimatedRuntimeSeconds: 300,
  estimatedTokens: 57600,
  coverageScore: 0.82,
  riskCoverage: 0.75,
  totalTestcases: 40,
  payloadStrategy: samplePayloadStrategy,
  recommendedPayloadStrategy: samplePayloadStrategy,
  plannerSource: "ai_runtime",
};

const sampleAuth = {
  kind: "bearer",
  engine: "header",
  method: "bearer",
  token: "sk-local-test",
};

afterEach(() => {
  clearScanConfigImport();
});

describe("scanConfigExport", () => {
  it("builds and parses a round-trip scan config with full attack plan", () => {
    const built = buildScanConfigExport({
      scanId: "scan-123",
      profile: sampleProfile,
      descriptor: { url: "https://api.example.com/v1/chat/completions", auth: sampleAuth },
      playbook: samplePlaybook,
    });

    expect(built.format).toBe(SCAN_CONFIG_FORMAT);
    expect(built.version).toBe(SCAN_CONFIG_VERSION);
    expect(built.source_scan_id).toBe("scan-123");
    expect(built.endpoint.url).toContain("api.example.com");
    expect(built.endpoint.headers.Authorization).toBe("Bearer sk-local-test");
    expect(built.auth).toEqual(sampleAuth);
    expect(built.attack.categories).toEqual(["prompt_injection", "jailbreak"]);
    expect(built.attack.agent_mode).toBe(true);
    expect(built.attack.max_agent_attempts).toBe(3);
    expect(built.attack.plan).toBeDefined();
    expect(built.attack.plan?.profileId).toBe("standard");
    expect(built.attack.plan?.attackGraph.length).toBe(2);
    expect(built.attack.plan?.executionStrategy).toBe("agentic");
    expect(built.attack.plan?.estimatedRequests).toBeGreaterThan(0);

    const parsed = parseScanConfigExport(JSON.stringify(built));
    expect(parsed.ok).toBe(true);
    if (!parsed.ok) return;

    expect(parsed.config.endpoint.base_url).toBe("https://api.example.com");
    expect(parsed.config.endpoint.path).toBe("/v1/chat/completions");
    expect(parsed.config.attack.disabled_tests).toEqual(["prompt_injection.basic"]);
    expect(parsed.config.auth).toEqual(sampleAuth);
    expect(parsed.config.attack.plan?.attackGraph).toHaveLength(2);

    const patch = scanConfigToWizardPatch(parsed.config, { projectId: "proj-1" });
    expect(patch.currentStep).toBe(2);
    expect(patch.targetProfile.baseUrl).toBe("https://api.example.com");
    expect(patch.targetForm.url).toContain("api.example.com");
    expect(patch.attackPlan).not.toBeNull();
    expect(patch.attackPlan?.categories).toContain("prompt_injection");
    expect(patch.attackPlan?.executionStrategy).toBe("agentic");
    expect(patch.attackPlan?.maxAttempts).toBe(3);
    expect(patch.attackPlan?.attackGraph.length).toBeGreaterThan(0);
    expect(patch.attackPlanUi.profileId).toBe("standard");
  });

  it("prefers wizard_snapshot.attackPlan for full step-5 hydrate", () => {
    const built = buildScanConfigExport({
      profile: sampleProfile,
      descriptor: { auth: sampleAuth },
      playbook: {
        ...samplePlaybook,
        wizard_snapshot: {
          version: 7,
          attackPlan: richWizardPlan,
        },
      },
    });

    expect(built.attack.plan?.profileId).toBe("deep");
    expect(built.attack.plan?.summary).toBe("Deep plan for example API");
    expect(built.attack.plan?.profileModes).toHaveLength(1);
    expect(built.attack.plan?.rationales).toHaveLength(1);
    expect(built.attack.plan?.estimatedRequests).toBe(120);
    expect(built.attack.categories).toEqual([
      "prompt_injection",
      "jailbreak",
      "system_prompt_extraction",
    ]);

    const patch = scanConfigToWizardPatch(built, { projectId: "proj-1" });
    expect(patch.attackPlan?.profileId).toBe("deep");
    expect(patch.attackPlan?.summary).toBe("Deep plan for example API");
    expect(patch.attackPlan?.estimatedRequests).toBe(120);
    expect(patch.attackPlan?.estimatedRuntimeSeconds).toBe(300);
    expect(patch.attackPlan?.attackGraph).toHaveLength(3);
    expect(patch.attackPlan?.attackGraph[1]?.dependencies).toContain("prompt_injection");
    expect(patch.attackPlan?.profileModes[0]?.profileId).toBe("deep");
    expect(patch.attackPlan?.rationales[0]?.reason).toContain("Open chat");
    expect(patch.attackPlan?.plannerSource).toBe("ai_runtime");
    expect(patch.attackPlanUi.profileId).toBe("deep");
  });

  it("rejects invalid JSON and wrong format/version", () => {
    expect(parseScanConfigExport("{").ok).toBe(false);
    expect(parseScanConfigExport(JSON.stringify({ format: "other", version: 1 })).ok).toBe(false);
    expect(
      parseScanConfigExport(
        JSON.stringify({
          format: SCAN_CONFIG_FORMAT,
          version: 99,
          endpoint: { url: "https://x.test/" },
          attack: { categories: ["prompt_injection"] },
        }),
      ).ok,
    ).toBe(false);
    expect(
      parseScanConfigExport(
        JSON.stringify({
          format: SCAN_CONFIG_FORMAT,
          version: SCAN_CONFIG_VERSION,
          endpoint: { url: "https://x.test/v1" },
          attack: { categories: [] },
        }),
      ).ok,
    ).toBe(false);
  });

  it("stashes import and hydrates a wizard session", () => {
    const built = buildScanConfigExport({
      profile: sampleProfile,
      descriptor: { auth: sampleAuth },
      playbook: samplePlaybook,
    });
    stashScanConfigImport(built);

    const session = createSessionFromScanConfigImport("project-1", { consume: true });
    expect(session.selectedProjectId).toBe("project-1");
    expect(session.currentStep).toBe(2);
    expect(session.targetProfile.baseUrl).toBe("https://api.example.com");
    expect(session.attackPlan).not.toBeNull();
    expect(session.attackPlanSource).toBe("imported");
    expect(session.attackPlan?.attackGraph.length).toBeGreaterThan(0);

    const remount = createSessionFromScanConfigImport("project-1", { consume: true });
    expect(remount.attackPlan).not.toBeNull();
    expect(remount.attackPlanSource).toBe("imported");
    expect(remount.targetProfile.baseUrl).toBe("https://api.example.com");

    clearScanConfigImport();
    const blank = createSessionFromScanConfigImport("project-1", { consume: true });
    expect(blank.attackPlan).toBeNull();
    expect(blank.attackPlanSource).toBeNull();
  });
});
