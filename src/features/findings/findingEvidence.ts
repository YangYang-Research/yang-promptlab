import type { Finding } from "@/shared/types";

export type JudgeRoleKey = "judge" | "classifier" | "attacker" | "other";

export type JudgeRoleResult = {
  id: string;
  role: JudgeRoleKey;
  label: string;
  kind: string | null;
  vulnerable: boolean;
  /** Confidence as 0–100 score points. */
  score: number;
  severity: string | null;
  category: string | null;
  rationale: string | null;
  indicators: string[];
};

export type JudgeConsensus = {
  agreementRatio: number | null;
  participating: number | null;
  vulnerableVotes: number | null;
  method: string | null;
  dissent: boolean | null;
};

export type JudgeScoreTerm = {
  label: string;
  role: JudgeRoleKey;
  kind: string | null;
  confidence: number;
  weight: number;
  weighted: number;
  vulnerable: boolean;
};

export type JudgeScoreBreakdown = {
  terms: JudgeScoreTerm[];
  weightTotal: number;
  weightedSum: number;
  base: number;
  agreement: number;
  vulnerableCount: number;
  totalCount: number;
  boost: number;
  final: number;
  finalScore: number;
  formula: string;
};

export type ParsedFindingEvidence = {
  payload: string | null;
  payloadId: string | null;
  requestUrl: string | null;
  requestMethod: string | null;
  requestHeaders: Record<string, string>;
  requestBody: string | null;
  responseStatus: number | null;
  responseHeaders: Record<string, string>;
  responseBody: string | null;
  responseExcerpt: string | null;
  explanation: string | null;
  indicators: string[];
  judgeSummary: string | null;
  judgeReasoning: string | null;
  judgeRoles: JudgeRoleResult[];
  judgeConsensus: JudgeConsensus | null;
  judgeScoreBreakdown: JudgeScoreBreakdown | null;
  /** Aggregate judge confidence as 0–100 score points. */
  judgeScore: number | null;
  /** ISO / RFC3339 timestamp from the last judge run, when present. */
  judgedAt: string | null;
  confidence: number | null;
  verdict: string | null;
  raw: unknown;
};

const ROLE_ORDER: JudgeRoleKey[] = ["judge", "classifier", "attacker", "other"];

const ROLE_LABELS: Record<JudgeRoleKey, string> = {
  judge: "JudgeWorker",
  classifier: "ClassifierWorker",
  attacker: "AttackerWorker",
  other: "Evaluator",
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

function asBool(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
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

function toScorePoints(confidence: number | null): number | null {
  if (confidence == null) return null;
  const normalized = confidence > 1 ? confidence : confidence * 100;
  return Math.round(Math.min(100, Math.max(0, normalized)));
}

function normalizeRole(value: unknown, evaluatorId: string | null): JudgeRoleKey {
  const raw = asString(value)?.toLowerCase().replace(/[\s-]/g, "_");
  if (raw === "judge" || raw === "classifier" || raw === "attacker") {
    return raw;
  }
  const id = evaluatorId?.toLowerCase() ?? "";
  if (id.includes("classifier")) return "classifier";
  if (id.includes("attacker")) return "attacker";
  if (id.includes("judge")) return "judge";
  return "other";
}

function roleLabel(role: JudgeRoleKey, evaluatorId: string | null, kind: string | null): string {
  if (role !== "other") return ROLE_LABELS[role];
  if (evaluatorId) return evaluatorId;
  if (kind) return kind.replace(/_/g, " ");
  return ROLE_LABELS.other;
}

export type JudgeRoleWeights = {
  judge: number;
  classifier: number;
  attacker: number;
  defaultLlm: number;
};

export const DEFAULT_JUDGE_ROLE_WEIGHTS: JudgeRoleWeights = {
  judge: 0.85,
  classifier: 0.8,
  attacker: 0.75,
  defaultLlm: 0.65,
};

function evaluatorWeight(role: JudgeRoleKey, weights: JudgeRoleWeights): number {
  if (role === "judge") return weights.judge;
  if (role === "classifier") return weights.classifier;
  if (role === "attacker") return weights.attacker;
  return weights.defaultLlm;
}

/** Recompute aggregate confidence the same way as `promptlab_judge::scoring::aggregate_confidence`. */
export function buildJudgeScoreBreakdown(
  roles: JudgeRoleResult[],
  weights: JudgeRoleWeights = DEFAULT_JUDGE_ROLE_WEIGHTS,
): JudgeScoreBreakdown | null {
  if (roles.length === 0) return null;

  const terms: JudgeScoreTerm[] = roles.map((role) => {
    const confidence = role.score / 100;
    const weight = evaluatorWeight(role.role, weights);
    return {
      label: role.label,
      role: role.role,
      kind: role.kind,
      confidence,
      weight,
      weighted: confidence * weight,
      vulnerable: role.vulnerable,
    };
  });

  const weightedSum = terms.reduce((sum, term) => sum + term.weighted, 0);
  const weightTotal = terms.reduce((sum, term) => sum + term.weight, 0);
  const base = weightTotal > 0 ? weightedSum / weightTotal : 0;
  const vulnerableCount = terms.filter((term) => term.vulnerable).length;
  const totalCount = terms.length;
  const agreement = totalCount > 0 ? vulnerableCount / totalCount : 0;
  const boost = agreement >= 0.66 ? 0.08 : agreement >= 0.5 ? 0.04 : 0;
  const final = Math.min(1, base + boost);
  const finalScore = Math.round(final * 100);

  const termExpr = terms
    .map((term) => `(${(term.confidence * 100).toFixed(0)}% × ${term.weight.toFixed(2)})`)
    .join(" + ");

  return {
    terms,
    weightTotal,
    weightedSum,
    base,
    agreement,
    vulnerableCount,
    totalCount,
    boost,
    final,
    finalScore,
    formula: `score = min(1, Σ(conf × w) / Σ(w) + boost) = min(1, ${termExpr} / ${weightTotal.toFixed(2)} + ${boost.toFixed(2)})`,
  };
}

function parseJudgeRoles(judge: Record<string, unknown> | null): JudgeRoleResult[] {
  if (!judge) return [];
  const rows = judge.evaluator_results ?? judge.evaluatorResults;
  if (!Array.isArray(rows)) return [];

  const parsed: JudgeRoleResult[] = [];
  for (const [index, row] of rows.entries()) {
    const item = asRecord(row);
    if (!item) continue;

    const evaluatorId = asString(item.evaluator_id) ?? asString(item.evaluatorId);
    const kind = asString(item.kind);
    const role = normalizeRole(item.role, evaluatorId);
    const confidence =
      asNumber(item.confidence) ??
      asNumber(asRecord(item.structured)?.confidence);
    const score = toScorePoints(confidence) ?? 0;
    const indicators = Array.isArray(item.indicators)
      ? item.indicators.filter((entry): entry is string => typeof entry === "string" && entry.trim() !== "")
      : [];

    parsed.push({
      id: evaluatorId ?? `${role}-${index}`,
      role,
      label: roleLabel(role, evaluatorId, kind),
      kind,
      vulnerable: asBool(item.vulnerable) ?? false,
      score,
      severity: asString(item.severity),
      category: asString(item.category),
      rationale: asString(item.rationale),
      indicators,
    });
  }

  return parsed.sort(
    (a, b) => ROLE_ORDER.indexOf(a.role) - ROLE_ORDER.indexOf(b.role) || a.label.localeCompare(b.label),
  );
}

function parseJudgeConsensus(judge: Record<string, unknown> | null): JudgeConsensus | null {
  const consensus = asRecord(judge?.consensus);
  if (!consensus) return null;

  return {
    agreementRatio:
      asNumber(consensus.agreement_ratio) ?? asNumber(consensus.agreementRatio),
    participating:
      asNumber(consensus.participating_evaluators) ??
      asNumber(consensus.participatingEvaluators),
    vulnerableVotes:
      asNumber(consensus.vulnerable_votes) ?? asNumber(consensus.vulnerableVotes),
    method: asString(consensus.method),
    dissent: asBool(consensus.dissent),
  };
}

function jsonStringFragment(content: string, template: string): string {
  const trimmed = template.trim();
  if (!(trimmed.startsWith("{") || trimmed.startsWith("["))) {
    return content;
  }
  try {
    const encoded = JSON.stringify(content);
    return encoded.slice(1, -1);
  } catch {
    return content;
  }
}

/** Rebuild the HTTP body that was actually sent (template + payload), matching attack runner. */
export function reconstructRequestBody(
  template: string | null,
  payload: string | null,
  storedBody: string | null,
): string | null {
  if (template?.trim() && payload != null) {
    const escaped = jsonStringFragment(payload, template);
    const injected = template
      .replaceAll("{{PROMPT}}", escaped)
      .replaceAll("{{payload}}", escaped);
    return prettyJsonIfPossible(injected);
  }

  if (storedBody?.trim()) {
    return prettyJsonIfPossible(storedBody);
  }

  if (payload?.trim()) {
    return prettyJsonIfPossible(payload);
  }

  if (template?.trim()) {
    return prettyJsonIfPossible(template);
  }

  return null;
}

function prettyJsonIfPossible(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

/** Strip leading blank/whitespace noise (SSE/stream padding) before the real body. */
export function sanitizeResponseBody(raw: string | null): string | null {
  if (raw == null) return null;
  const trimmed = raw.trim();
  if (!trimmed) return null;

  const jsonStart = trimmed.search(/[{\[]/);
  if (jsonStart > 0) {
    const candidate = trimmed.slice(jsonStart);
    try {
      return JSON.stringify(JSON.parse(candidate), null, 2);
    } catch {
      // fall through — keep trimmed full text
    }
  }

  return prettyJsonIfPossible(trimmed);
}

function parseHeaderMap(value: unknown): Record<string, string> {
  const record = asRecord(value);
  if (!record) return {};
  const headers: Record<string, string> = {};
  for (const [key, entry] of Object.entries(record)) {
    if (typeof entry === "string" && key.trim()) {
      headers[key] = entry;
    } else if (typeof entry === "number" || typeof entry === "boolean") {
      headers[key] = String(entry);
    }
  }
  return headers;
}

function hasHeader(headers: Record<string, string>, name: string): boolean {
  const needle = name.toLowerCase();
  return Object.keys(headers).some((key) => key.toLowerCase() === needle);
}

function redactHeaderValue(name: string, value: string): string {
  const key = name.toLowerCase();
  if (
    key === "authorization" ||
    key === "proxy-authorization" ||
    key === "cookie" ||
    key === "set-cookie" ||
    key.includes("api-key") ||
    key.includes("apikey") ||
    key.includes("token")
  ) {
    return "[REDACTED]";
  }
  return value;
}

function formatHeaderBlock(headers: Record<string, string>): string[] {
  return Object.entries(headers)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([name, value]) => `${name}: ${redactHeaderValue(name, value)}`);
}

function parseRequestUrl(url: string | null): {
  host: string | null;
  pathWithQuery: string;
} {
  if (!url?.trim()) {
    return { host: null, pathWithQuery: "/" };
  }
  try {
    const parsed = new URL(url);
    const pathWithQuery = `${parsed.pathname || "/"}${parsed.search}`;
    const host = parsed.port ? `${parsed.hostname}:${parsed.port}` : parsed.hostname;
    return { host, pathWithQuery };
  } catch {
    return { host: null, pathWithQuery: url };
  }
}

/** Render a wire-style HTTP request from finding evidence. */
export function formatHttpRequest(evidence: ParsedFindingEvidence): string {
  const method = (evidence.requestMethod ?? "POST").trim().toUpperCase() || "POST";
  const { host, pathWithQuery } = parseRequestUrl(evidence.requestUrl);
  const headers = { ...evidence.requestHeaders };
  if (host && !hasHeader(headers, "Host")) {
    headers.Host = host;
  }
  if (evidence.requestBody?.trim() && !hasHeader(headers, "Content-Type")) {
    headers["Content-Type"] = "application/json";
  }

  const lines = [`${method} ${pathWithQuery} HTTP/1.1`, ...formatHeaderBlock(headers)];
  const body = evidence.requestBody?.trim();
  if (body) {
    lines.push("", body);
  }
  return lines.join("\n");
}

/** Render a wire-style HTTP response from finding evidence. */
export function formatHttpResponse(evidence: ParsedFindingEvidence): string {
  const status = evidence.responseStatus;
  const statusLine =
    status == null || status === 0
      ? "HTTP/1.1 000"
      : `HTTP/1.1 ${status}`;
  const lines = [statusLine, ...formatHeaderBlock(evidence.responseHeaders)];
  const body = (evidence.responseBody ?? evidence.responseExcerpt)?.trim();
  if (body) {
    lines.push("", body);
  } else if (status == null && Object.keys(evidence.responseHeaders).length === 0) {
    return "—";
  }
  return lines.join("\n");
}

export function parseFindingEvidence(
  finding: Pick<Finding, "evidence"> & { description?: string },
  weights: JudgeRoleWeights = DEFAULT_JUDGE_ROLE_WEIGHTS,
): ParsedFindingEvidence {
  const raw = finding.evidence;
  const obj = asRecord(raw);
  const request = asRecord(obj?.request);
  const response = asRecord(obj?.response);
  const judge = asRecord(obj?.judge);

  const indicatorsFromRoot = Array.isArray(obj?.indicators)
    ? obj.indicators.filter((item): item is string => typeof item === "string")
    : [];
  const indicatorsFromJudge = Array.isArray(judge?.evidence)
    ? judge.evidence.filter((item): item is string => typeof item === "string")
    : [];
  const judgeRoles = parseJudgeRoles(judge);
  const scoreBreakdown = buildJudgeScoreBreakdown(judgeRoles, weights);
  const indicatorsFromEvaluators = judgeRoles.flatMap((role) => role.indicators);
  const indicators =
    indicatorsFromRoot.length > 0
      ? indicatorsFromRoot
      : indicatorsFromJudge.length > 0
        ? indicatorsFromJudge
        : indicatorsFromEvaluators;

  const confidence = asNumber(obj?.confidence) ?? asNumber(judge?.confidence);
  const judgeScore =
    scoreBreakdown?.finalScore ?? toScorePoints(confidence);

  const payload = asString(obj?.payload);
  const bodyTemplate =
    asString(request?.body_template) ?? asString(request?.bodyTemplate);
  // Storage currently puts the attack payload in request.body, not the wire HTTP body.
  const storedRequestBody = asString(request?.body) ?? asString(obj?.request_body);
  const requestBody = reconstructRequestBody(bodyTemplate, payload, storedRequestBody);

  return {
    payload,
    payloadId: asString(obj?.payload_id) ?? asString(obj?.payloadId),
    requestUrl: asString(request?.url) ?? asString(obj?.endpoint),
    requestMethod: asString(request?.method),
    requestHeaders: parseHeaderMap(request?.headers),
    requestBody,
    responseStatus: asNumber(response?.status) ?? asNumber(obj?.response_status),
    responseHeaders: parseHeaderMap(response?.headers),
    responseBody: sanitizeResponseBody(
      asString(response?.body) ??
        asString(obj?.response_body) ??
        asString(response?.normalized) ??
        asString(obj?.response_excerpt),
    ),
    responseExcerpt: sanitizeResponseBody(asString(obj?.response_excerpt)),
    explanation:
      asString(obj?.explanation) ??
      asString(judge?.summary) ??
      asString(judge?.reasoning) ??
      (finding.description || null),
    indicators,
    judgeSummary: asString(judge?.summary),
    judgeReasoning: asString(judge?.reasoning) ?? formatJsonBlock(judge?.analysis),
    judgeRoles,
    judgeConsensus: parseJudgeConsensus(judge),
    judgeScoreBreakdown: scoreBreakdown,
    judgeScore,
    judgedAt: asString(judge?.judged_at) ?? asString(judge?.judgedAt),
    confidence,
    verdict: asString(obj?.verdict) ?? asString(judge?.verdict),
    raw,
  };
}

function shellSingleQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

/** Build a reproducible curl from finding request evidence (auth token redacted). */
export function buildFindingCurl(evidence: Pick<
  ParsedFindingEvidence,
  "requestMethod" | "requestUrl" | "requestBody"
>): string | null {
  const url = evidence.requestUrl?.trim();
  if (!url) return null;

  const method = (evidence.requestMethod ?? "POST").trim().toUpperCase() || "POST";
  const parts = [`curl -sS -X ${method} ${shellSingleQuote(url)}`];
  parts.push(`  -H ${shellSingleQuote("Content-Type: application/json")}`);
  parts.push(`  -H ${shellSingleQuote("Authorization: Bearer $API_TOKEN")}`);

  const body = evidence.requestBody?.trim();
  if (body) {
    parts.push(`  --data-raw ${shellSingleQuote(body)}`);
  }

  return parts.join(" \\\n");
}
