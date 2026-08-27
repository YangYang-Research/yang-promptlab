import { describe, expect, it } from "vitest";

import {
  asGlobalSearchHit,
  GLOBAL_SEARCH_KIND_LABEL,
  groupSearchHits,
  type GlobalSearchHit,
} from "@/app/layout/globalSearch";

describe("asGlobalSearchHit", () => {
  it("accepts routed hits from the database search command", () => {
    const hit = asGlobalSearchHit({
      id: "project:abc",
      kind: "project",
      title: "Acme",
      subtitle: "prod",
      to: "/projects/abc",
    });
    expect(hit?.kind).toBe("project");
    expect(GLOBAL_SEARCH_KIND_LABEL[hit!.kind]).toBe("Projects");
  });

  it("rejects unknown kinds and non-app paths", () => {
    expect(
      asGlobalSearchHit({
        id: "x",
        kind: "payload",
        title: "x",
        subtitle: "",
        to: "/projects/x",
      }),
    ).toBeNull();
    expect(
      asGlobalSearchHit({
        id: "x",
        kind: "project",
        title: "x",
        subtitle: "",
        to: "https://evil.example",
      }),
    ).toBeNull();
    expect(
      asGlobalSearchHit({
        id: "technique:pi-direct-override",
        kind: "technique",
        title: "Direct instruction override",
        subtitle: "LLM01",
        to: "/attack-categories/pi-direct-override",
      })?.kind,
    ).toBe("technique");
    expect(GLOBAL_SEARCH_KIND_LABEL.technique).toBe("Attack Techniques");
    expect(
      asGlobalSearchHit({
        id: "x",
        kind: "project",
        title: "x",
        subtitle: "",
        to: "/models/x",
      }),
    ).toBeNull();
  });
});

describe("groupSearchHits", () => {
  it("puts a category header group above its hits in stable kind order", () => {
    const hits: GlobalSearchHit[] = [
      {
        id: "finding:1",
        kind: "finding",
        title: "Leak",
        subtitle: "critical",
        to: "/findings/1",
      },
      {
        id: "project:1",
        kind: "project",
        title: "Acme",
        subtitle: "prod",
        to: "/projects/1",
      },
      {
        id: "project:2",
        kind: "project",
        title: "Beta",
        subtitle: "",
        to: "/projects/2",
      },
      {
        id: "technique:1",
        kind: "technique",
        title: "Direct override",
        subtitle: "LLM01",
        to: "/attack-categories/pi-direct-override",
      },
    ];

    const groups = groupSearchHits(hits);
    expect(groups.map((group) => group.label)).toEqual([
      "Projects",
      "Findings",
      "Attack Techniques",
    ]);
    expect(groups[0].hits.map((hit) => hit.title)).toEqual(["Acme", "Beta"]);
    expect(groups[1].hits.map((hit) => hit.title)).toEqual(["Leak"]);
  });
});
