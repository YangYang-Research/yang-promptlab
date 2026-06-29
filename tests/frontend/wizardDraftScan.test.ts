import { describe, expect, it, vi } from "vitest";

import { findWizardDraftScan, resolveOrCreateDraftScanId } from "@/features/scans/wizardDraftScan";
import type { ScanRun } from "@/shared/types";

describe("wizardDraftScan", () => {
  it("finds the newest draft for a project", () => {
    const scans: ScanRun[] = [
      {
        id: "old",
        projectId: "p1",
        targetId: null,
        name: "Setup Scan",
        status: "draft",
        startedAt: null,
        completedAt: null,
        createdAt: "2026-01-01T00:00:00Z",
      },
      {
        id: "new",
        projectId: "p1",
        targetId: null,
        name: "Setup Scan",
        status: "draft",
        startedAt: null,
        completedAt: null,
        createdAt: "2026-01-02T00:00:00Z",
      },
    ];

    expect(findWizardDraftScan(scans, "p1")?.id).toBe("new");
  });

  it("deduplicates concurrent draft creation per project", async () => {
    const factory = vi.fn(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
      return "scan-1";
    });

    const [a, b] = await Promise.all([
      resolveOrCreateDraftScanId("project-1", factory),
      resolveOrCreateDraftScanId("project-1", factory),
    ]);

    expect(a).toBe("scan-1");
    expect(b).toBe("scan-1");
    expect(factory).toHaveBeenCalledTimes(1);
  });
});
