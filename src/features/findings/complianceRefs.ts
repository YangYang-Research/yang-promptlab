/** Mirrors `aisec_report::recommendations::compliance_refs_for`. */
export function complianceRefsFor(category: string): string[] {
  const cat = category.toLowerCase();
  const refs = ["OWASP LLM Top 10"];

  if (cat.includes("injection") || cat.includes("prompt")) {
    refs.push("LLM01: Prompt Injection");
    refs.push("NIST AI RMF: MAP 1.5");
  } else if (cat.includes("jailbreak")) {
    refs.push("LLM02: Insecure Output Handling");
  } else if (cat.includes("rag")) {
    refs.push("LLM06: Sensitive Information Disclosure");
  } else if (cat.includes("tool") || cat.includes("mcp") || cat.includes("agent")) {
    refs.push("LLM08: Excessive Agency");
  } else {
    refs.push("LLM09: Overreliance");
  }

  return refs;
}
