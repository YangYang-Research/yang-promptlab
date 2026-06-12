import { describe, expect, it } from "vitest";

import {
  DISCOVERY_PHASES,
  endpointSourceLabel,
  phaseStatuses,
} from "@/features/scans/discoveryPhases";

describe("phaseStatuses", () => {
  it("marks all complete when discovery finished", () => {
    expect(phaseStatuses(false, true, 0)).toEqual(
      DISCOVERY_PHASES.map(() => "complete"),
    );
  });

  it("highlights active phase while running", () => {
    const statuses = phaseStatuses(true, false, 2);
    expect(statuses[0]).toBe("complete");
    expect(statuses[1]).toBe("complete");
    expect(statuses[2]).toBe("active");
    expect(statuses[3]).toBe("pending");
  });
});

describe("endpointSourceLabel", () => {
  it("detects manual endpoints", () => {
    expect(endpointSourceLabel("manual", null)).toBe("Manual");
    expect(endpointSourceLabel("rest_api", "manual")).toBe("Manual");
  });

  it("labels discovered endpoints", () => {
    expect(endpointSourceLabel("rest_api", "https://example.com")).toBe("Discovery");
  });
});
