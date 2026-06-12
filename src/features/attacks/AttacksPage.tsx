import { useEffect, useMemo, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  Card,
  DataTable,
  EmptyState,
  PageHeader,
  SeverityBadge,
  StatusBadge,
} from "@/shared/components";
import { useToast } from "@/shared/notifications";
import type { Finding } from "@/shared/types";

const attackCategories = [
  { id: "prompt_injection", label: "Prompt Injection", enabled: true },
  { id: "jailbreak", label: "Jailbreak", enabled: false },
  { id: "system_prompt_leak", label: "System Prompt Leak", enabled: false },
  { id: "data_exfiltration", label: "Data Exfiltration", enabled: false },
] as const;

const findingColumns = [
  {
    key: "severity",
    header: "Severity",
    width: "100px",
    render: (f: Finding) => <SeverityBadge severity={f.severity} />,
  },
  {
    key: "title",
    header: "Finding",
    render: (f: Finding) => (
      <div>
        <strong>{f.title}</strong>
        <div className="text-muted text-sm">{f.description}</div>
      </div>
    ),
  },
  {
    key: "confidence",
    header: "Confidence",
    width: "100px",
    render: (f: Finding) => `${Math.round(f.confidence * 100)}%`,
  },
  {
    key: "status",
    header: "Status",
    width: "110px",
    render: (f: Finding) => <Badge variant="muted">{f.status}</Badge>,
  },
];

export function AttacksPage() {
  const { endpoints, scans, findings, targets, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [endpointId, setEndpointId] = useState("");
  const [category, setCategory] = useState("prompt_injection");
  const [running, setRunning] = useState(false);

  // Prefer AI endpoints, but allow attacking any discovered endpoint.
  const attackable = useMemo(() => {
    const ai = endpoints.filter((e) => e.kind === "ai_endpoint");
    return ai.length > 0 ? ai.concat(endpoints.filter((e) => e.kind !== "ai_endpoint")) : endpoints;
  }, [endpoints]);

  useEffect(() => {
    if (!endpointId && attackable.length > 0) {
      setEndpointId(attackable[0].id);
    }
  }, [attackable, endpointId]);

  const findingsByScan = useMemo(() => {
    const map = new Map<string, Finding[]>();
    for (const f of findings) {
      const list = map.get(f.scanId) ?? [];
      list.push(f);
      map.set(f.scanId, list);
    }
    return map;
  }, [findings]);

  // Attack runs are scans created by the prompt-injection command.
  const runs = useMemo(
    () =>
      scans
        .filter((s) => s.name.startsWith("Prompt Injection:"))
        .sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [scans],
  );

  const targetName = (id: string | null) =>
    id ? targets.find((t) => t.id === id)?.name ?? "—" : "—";

  async function handleLaunch() {
    if (!endpointId || running || category !== "prompt_injection") return;
    setRunning(true);
    try {
      const result = await actions.runPromptInjection(endpointId);
      notify(
        result.findings.length > 0
          ? `Attack complete — ${result.findings.length} finding(s) from ${result.attempts} attempt(s)`
          : `Attack complete — ${result.attempts} attempt(s), no findings`,
        result.findings.length > 0 ? "success" : "info",
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : "Attack failed";
      notify(message, "error");
    } finally {
      setRunning(false);
    }
  }

  const noEndpoints = endpoints.length === 0;
  const selectedEndpoint = endpoints.find((e) => e.id === endpointId) ?? null;

  return (
    <div className="page">
      <PageHeader
        title="Attacks"
        description="OWASP LLM Top 10 aligned attack orchestration"
        actions={
          <div className="discovery-controls">
            <select
              className="input"
              value={endpointId}
              onChange={(e) => setEndpointId(e.target.value)}
              disabled={noEndpoints || running}
            >
              {noEndpoints && <option value="">No endpoints — run Discovery first</option>}
              {attackable.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.kind} · {e.url}
                </option>
              ))}
            </select>
            <select
              className="input"
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              disabled={running}
            >
              {attackCategories.map((c) => (
                <option key={c.id} value={c.id} disabled={!c.enabled}>
                  {c.label}
                  {c.enabled ? "" : " (soon)"}
                </option>
              ))}
            </select>
            <Button
              variant="primary"
              onClick={handleLaunch}
              disabled={noEndpoints || running || !endpointId}
            >
              {running ? "Attacking…" : "Launch Attack"}
            </Button>
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">Failed to load attack data: {error}</p>
        </Card>
      )}

      {running && (
        <Card>
          <div className="discovery-progress__row">
            <div className="page-loader__spinner" />
            <div>
              <strong>Running prompt injection against {selectedEndpoint?.url}…</strong>
              <p className="text-muted text-sm">
                Sending payloads over HTTP and evaluating responses for injection indicators.
              </p>
            </div>
          </div>
        </Card>
      )}

      {runs.length === 0 && !running ? (
        <EmptyState
          title={loading ? "Loading…" : "No attack runs yet"}
          description={
            noEndpoints
              ? "Run Discovery to find endpoints, then launch an attack here."
              : "Select a discovered endpoint and launch a Prompt Injection attack."
          }
        />
      ) : (
        <div className="discovery-runs">
          {runs.map((run) => {
            const runFindings = findingsByScan.get(run.id) ?? [];
            return (
              <Card key={run.id} className="discovery-card">
                <div className="discovery-card__header">
                  <div>
                    <h3 className="discovery-card__title">
                      Prompt Injection · {targetName(run.targetId)}
                    </h3>
                    <p className="text-muted text-sm">
                      {run.completedAt
                        ? `Completed ${new Date(run.completedAt).toLocaleString()}`
                        : `Created ${new Date(run.createdAt).toLocaleString()}`}
                    </p>
                  </div>
                  <StatusBadge status={run.status} />
                </div>

                <div className="discovery-card__stats">
                  <div>
                    <span className="discovery-card__stat-value">{runFindings.length}</span>
                    <span className="discovery-card__stat-label">Findings</span>
                  </div>
                  <div>
                    <span className="discovery-card__stat-value">
                      {runFindings.filter((f) => f.severity === "critical").length}
                    </span>
                    <span className="discovery-card__stat-label">Critical</span>
                  </div>
                </div>

                {runFindings.length > 0 ? (
                  <DataTable
                    columns={findingColumns}
                    rows={runFindings}
                    keyField="id"
                    emptyMessage="No findings"
                  />
                ) : (
                  <p className="text-muted text-sm">
                    No injection indicators detected for this endpoint.
                  </p>
                )}
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
