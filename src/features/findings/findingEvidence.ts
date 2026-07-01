import type { Finding } from "@/shared/types";

export type ParsedFindingEvidence = {
  payload: string | null;
  payloadId: string | null;
  requestUrl: string | null;
  requestMethod: string | null;
  requestBody: string | null;
  responseStatus: number | null;
  responseBody: string | null;
  responseExcerpt: string | null;
  explanation: string | null;
  indicators: string[];
  judgeSummary: string | null;
  judgeReasoning: string | null;
  confidence: number | null;
  verdict: string | null;
  raw: unknown;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function formatJsonBlock(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function parseFindingEvidence(finding: Pick<Finding, "description" | "evidence">): ParsedFindingEvidence {
  const raw = finding.evidence;
  const obj = asRecord(raw);
  const request = asRecord(obj?.request);
  const response = asRecord(obj?.response);
  const judge = asRecord(obj?.judge);

  const indicators = Array.isArray(obj?.indicators)
    ? obj.indicators.filter((item): item is string => typeof item === "string")
    : [];

  return {
    payload: asString(obj?.payload),
    payloadId: asString(obj?.payload_id) ?? asString(obj?.payloadId),
    requestUrl: asString(request?.url) ?? asString(obj?.endpoint),
    requestMethod: asString(request?.method),
    requestBody:
      asString(request?.body) ??
      asString(request?.body_template) ??
      asString(obj?.request_body),
    responseStatus: asNumber(response?.status) ?? asNumber(obj?.response_status),
    responseBody:
      asString(response?.body) ??
      asString(obj?.response_body) ??
      asString(obj?.response_excerpt),
    responseExcerpt: asString(obj?.response_excerpt),
    explanation:
      asString(obj?.explanation) ??
      asString(judge?.summary) ??
      asString(judge?.reasoning) ??
      (finding.description || null),
    indicators,
    judgeSummary: asString(judge?.summary),
    judgeReasoning: asString(judge?.reasoning) ?? formatJsonBlock(judge?.analysis),
    confidence: asNumber(obj?.confidence) ?? asNumber(judge?.confidence),
    verdict: asString(obj?.verdict),
    raw,
  };
}
