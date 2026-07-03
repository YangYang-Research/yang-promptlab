import { describe, expect, it } from "vitest";

import { formatScanProgressPercent } from "@/features/scans/scanProgressFormat";

describe("formatScanProgressPercent", () => {
  it("rounds to two decimal places", () => {
    expect(formatScanProgressPercent(2.248126561199)).toBe("2.25%");
    expect(formatScanProgressPercent(100)).toBe("100.00%");
  });
});
