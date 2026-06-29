import { describe, expect, it } from "vitest";

import { createInitialTargetForm } from "@/features/scans/targetDescriptor";
import { mergeWizardSessions } from "@/features/scans/wizardPersistence";
import { createInitialSession } from "@/features/scans/wizardState";

describe("mergeWizardSessions", () => {
  it("keeps local auth secrets when remote draft lost them", () => {
    const local = {
      ...createInitialSession("p1"),
      draftScanId: "scan-1",
      currentStep: 3 as const,
      targetForm: {
        ...createInitialTargetForm(),
        authKind: "api_key" as const,
        apiKeyHeaderName: "Authorization",
        apiKeyValue: "sk-local-secret",
      },
    };
    const remote = {
      ...createInitialSession("p1"),
      draftScanId: "scan-1",
      currentStep: 3 as const,
      targetForm: {
        ...createInitialTargetForm(),
        authKind: "api_key" as const,
        apiKeyHeaderName: "Authorization",
        apiKeyValue: "",
      },
    };

    const merged = mergeWizardSessions(local, remote);
    expect(merged.targetForm.apiKeyValue).toBe("sk-local-secret");
  });
});
