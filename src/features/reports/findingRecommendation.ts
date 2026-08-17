/** Mirrors `promptlab_report::recommendations::recommendation_for`. */
export function recommendationFor(category: string): { title: string; description: string } {
  const cat = category.toLowerCase();
  if (cat.includes("injection") || cat.includes("prompt")) {
    return {
      title: "Mitigate prompt injection",
      description:
        "Isolate system instructions, validate user input, and apply output filtering.",
    };
  }
  if (cat.includes("jailbreak")) {
    return {
      title: "Address jailbreak vulnerability",
      description:
        "Update safety policies and add classifier layers for roleplay bypass attempts.",
    };
  }
  if (cat.includes("rag")) {
    return {
      title: "Fix RAG leakage",
      description: "Restrict context exposure and validate retrieval scope per user session.",
    };
  }
  if (cat.includes("tool") || cat.includes("mcp")) {
    return {
      title: "Lock down tool access",
      description: "Validate tool parameters and enforce authorization on agent actions.",
    };
  }
  return {
    title: "Remediate AI security finding",
    description:
      "Review finding evidence and apply appropriate guardrails for this attack category.",
  };
}
