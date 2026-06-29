import { describe, expect, it } from "vitest";

import {
  normalizeVerifiedAt,
  normalizeVerification,
  profileToPayload,
  createInitialTargetProfile,
} from "@/features/scans/targetProfile";

describe("normalizeVerifiedAt", () => {
  it("accepts RFC3339 strings", () => {
    expect(normalizeVerifiedAt("2025-06-13T12:00:00Z")).toBe("2025-06-13T12:00:00Z");
  });

  it("drops legacy array timestamps from DB JSON", () => {
    expect(normalizeVerifiedAt([2025, 181, 12, 0, 0, 0, 0, 0, 0])).toBeNull();
  });

  it("drops empty strings", () => {
    expect(normalizeVerifiedAt("  ")).toBeNull();
  });
});

describe("profileToPayload verification", () => {
  it("never sends verifiedAt as an array", () => {
    const form = createInitialTargetProfile();
    form.verification = normalizeVerification({
      ...form.verification,
      verified: true,
      verifiedAt: [2025, 181, 12, 0, 0, 0, 0, 0, 0] as unknown as string,
    });

    const payload = profileToPayload(form) as {
      verification: { verifiedAt: unknown };
    };

    expect(Array.isArray(payload.verification.verifiedAt)).toBe(false);
    expect(payload.verification.verifiedAt).toBeNull();
  });
});
