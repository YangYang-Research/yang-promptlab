export const HTTP_METHODS = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "OPTIONS",
  "HEAD",
] as const;

export type HttpMethod = (typeof HTTP_METHODS)[number];

const POST_KEYWORDS = [
  "chat",
  "completion",
  "generate",
  "predict",
  "invoke",
  "messages",
  "agent",
  "workflow",
  "ask",
  "query",
  "prompt",
] as const;

const GET_KEYWORDS = [
  "health",
  "status",
  "metrics",
  "version",
  "swagger",
  "openapi",
] as const;

export function inferEndpointMethod(pathOrUrl: string): HttpMethod {
  const lower = pathOrUrl.toLowerCase();
  for (const keyword of POST_KEYWORDS) {
    if (lower.includes(keyword)) return "POST";
  }
  for (const keyword of GET_KEYWORDS) {
    if (lower.includes(keyword)) return "GET";
  }
  return "GET";
}

export function endpointPath(url: string): string {
  try {
    return new URL(url).pathname;
  } catch {
    return url;
  }
}

export function confidenceLabel(confidence: number): "High" | "Medium" | "Low" {
  if (confidence >= 0.75) return "High";
  if (confidence >= 0.45) return "Medium";
  return "Low";
}
