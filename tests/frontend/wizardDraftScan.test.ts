import { describe, expect, it, vi } from "vitest";

import {
  canReuseWizardDraft,
  findWizardDraftScan,
  resolveOrCreateDraftScanId,
} from "@/features/scans/wizardDraftScan";
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

  it("does not fall back to another target draft when targetId is set", () => {
    const scans: ScanRun[] = [
      {
        id: "draft-a",
        projectId: "p1",
        targetId: "target-a",
        name: "Setup Scan",
        status: "draft",
        startedAt: null,
        completedAt: null,
        createdAt: "2026-01-02T00:00:00Z",
      },
      {
        id: "draft-b",
        projectId: "p1",
        targetId: "target-b",
        name: "Setup Scan",
        status: "draft",
        startedAt: null,
        completedAt: null,
        createdAt: "2026-01-01T00:00:00Z",
      },
    ];

    expect(findWizardDraftScan(scans, "p1", "target-b")?.id).toBe("draft-b");
    expect(findWizardDraftScan(scans, "p1", "target-c")).toBeNull();
  });

  it("deduplicates concurrent draft creation per project+target", async () => {
    const factory = vi.fn(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
      return "scan-1";
    });

    const [a, b] = await Promise.all([
      resolveOrCreateDraftScanId("project-1", factory, "target-a"),
      resolveOrCreateDraftScanId("project-1", factory, "target-a"),
    ]);

    expect(a).toBe("scan-1");
    expect(b).toBe("scan-1");
    expect(factory).toHaveBeenCalledTimes(1);
  });

  it("allows parallel draft creation for different targets", async () => {
    let n = 0;
    const factoryA = vi.fn(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
      return `scan-a-${++n}`;
    });
    const factoryB = vi.fn(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
      return `scan-b-${++n}`;
    });

    const [a, b] = await Promise.all([
      resolveOrCreateDraftScanId("project-1", factoryA, "target-a"),
      resolveOrCreateDraftScanId("project-1", factoryB, "target-b"),
    ]);

    expect(a).not.toBe(b);
    expect(factoryA).toHaveBeenCalledTimes(1);
    expect(factoryB).toHaveBeenCalledTimes(1);
  });

  it("refuses to reuse draft from another target on New Scan entry", () => {
    expect(
      canReuseWizardDraft({
        draftScanId: "draft-a",
        sessionTargetId: "target-a",
        lockedTargetId: "target-b",
        entryStep: 2,
        draftScanTargetId: "target-a",
      }),
    ).toBe(false);

    expect(
      canReuseWizardDraft({
        draftScanId: "draft-b",
        sessionTargetId: "target-b",
        lockedTargetId: "target-b",
        entryStep: 3,
        draftScanTargetId: "target-b",
      }),
    ).toBe(true);
  });
});
