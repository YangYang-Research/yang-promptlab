import { describe, expect, it } from "vitest";

import { resolveScanOpenPath } from "@/features/scans/wizardState";
import type { ScanRun } from "@/shared/types";

const baseScan: ScanRun = {
  id: "scan-1",
  projectId: "project-1",
  targetId: "target-1",
  name: "Scan (standard)",
  status: "completed",
  startedAt: null,
  completedAt: null,
  createdAt: "2026-07-09T00:00:00.000Z",
};

describe("resolveScanOpenPath", () => {
  it("opens wizard step 5 for active scans", () => {
    const path = resolveScanOpenPath({ ...baseScan, status: "running" }, "running");
    expect(path).toBe("/scans/new?projectId=project-1&targetId=target-1&scanId=scan-1&step=5");
  });

  it("ignores stale live status when store scan is completed", () => {
    const path = resolveScanOpenPath({ ...baseScan, status: "completed" }, "running");
    expect(path).toBe("/scans/scan-1");
  });

  it("opens wizard step 4 for failed scans", () => {
    const path = resolveScanOpenPath({ ...baseScan, status: "failed" });
    expect(path).toBe("/scans/new?projectId=project-1&targetId=target-1&scanId=scan-1&step=4");
  });

  it("opens scan details for completed scans", () => {
    const path = resolveScanOpenPath(baseScan);
    expect(path).toBe("/scans/scan-1");
  });

  it("opens wizard resume for drafts", () => {
    const path = resolveScanOpenPath({ ...baseScan, status: "draft" });
    expect(path).toBe("/scans/new?projectId=project-1&targetId=target-1&scanId=scan-1");
  });
});
