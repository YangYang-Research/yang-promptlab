import { describe, expect, it } from "vitest";

import {
  confidenceLabel,
  inferEndpointMethod,
} from "@/features/scans/endpointMethod";

describe("inferEndpointMethod", () => {
  it("defaults chat-like paths to POST", () => {
    expect(inferEndpointMethod("/v1/chat/completions")).toBe("POST");
    expect(inferEndpointMethod("/api/generate")).toBe("POST");
  });

  it("defaults health-like paths to GET", () => {
    expect(inferEndpointMethod("/health")).toBe("GET");
    expect(inferEndpointMethod("/metrics")).toBe("GET");
  });

  it("defaults unknown paths to GET", () => {
    expect(inferEndpointMethod("/api/users")).toBe("GET");
  });
});

describe("confidenceLabel", () => {
  it("maps score bands to labels", () => {
    expect(confidenceLabel(0.9)).toBe("High");
    expect(confidenceLabel(0.5)).toBe("Medium");
    expect(confidenceLabel(0.2)).toBe("Low");
  });
});
