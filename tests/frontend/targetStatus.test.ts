import { describe, expect, it } from "vitest";

import {
  deriveTargetLastScanAt,
  deriveTargetStatus,
} from "@/shared/targetScanContext";
import type { ScanRun } from "@/shared/types";

function scan(partial: Partial<ScanRun> & Pick<ScanRun, "id" | "status">): ScanRun {
  return {
    id: partial.id,
    projectId: "proj-1",
    targetId: partial.targetId ?? "target-1",
    name: partial.name ?? "Scan (standard)",
    status: partial.status,
    startedAt: partial.startedAt ?? "2026-07-01T10:00:00.000Z",
    completedAt: partial.completedAt ?? null,
    createdAt: partial.createdAt ?? "2026-07-01T09:00:00.000Z",
  };
}

describe("deriveTargetStatus", () => {
  it("returns pending when profile is not verified and no scans", () => {
    expect(deriveTargetStatus({}, "target-1", [])).toBe("pending");
    expect(
      deriveTargetStatus({ verification: { verified: false } }, "target-1", []),
    ).toBe("pending");
  });

  it("returns verified when profile is verified and no finished scan", () => {
    expect(
      deriveTargetStatus({ verification: { verified: true } }, "target-1", []),
    ).toBe("verified");
    expect(
      deriveTargetStatus(
        { verification: { verified: true } },
        "target-1",
        [scan({ id: "s1", status: "running" })],
      ),
    ).toBe("verified");
  });

  it("returns scanned when a finished attack scan exists", () => {
    expect(
      deriveTargetStatus(
        { verification: { verified: true } },
        "target-1",
        [
          scan({
            id: "s1",
            status: "completed",
            completedAt: "2026-07-01T11:00:00.000Z",
          }),
        ],
      ),
    ).toBe("scanned");
  });

  it("counts Agent Scan names as attack scans", () => {
    expect(
      deriveTargetStatus(
        { verification: { verified: true } },
        "target-1",
        [scan({ id: "s1", status: "completed", name: "Agent Scan (deep)" })],
      ),
    ).toBe("scanned");
  });
});

describe("deriveTargetLastScanAt", () => {
  it("returns completedAt from the latest finished attack scan", () => {
    expect(
      deriveTargetLastScanAt("target-1", [
        scan({
          id: "s1",
          status: "completed",
          createdAt: "2026-07-01T09:00:00.000Z",
          completedAt: "2026-07-01T10:00:00.000Z",
        }),
        scan({
          id: "s2",
          status: "completed",
          createdAt: "2026-07-02T09:00:00.000Z",
          completedAt: "2026-07-02T10:00:00.000Z",
        }),
      ]),
    ).toBe("2026-07-02T10:00:00.000Z");
  });
});
