import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  DataTable,
  ProgressBar,
} from "@/shared/components";
import type { EndpointDto, DiscoveryStatsDto } from "@/shared/ipc";
import { createEndpoint, listEndpoints } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import type { Target } from "@/shared/types";

import {
  DISCOVERY_PHASES,
  endpointSourceLabel,
  phaseStatuses,
} from "../discoveryPhases";

export type DiscoverySelection = {
  scanId: string | null;
  selectedCount: number;
  selectedEndpointIds: string[];
};

type DiscoveryStepProps = {
  target: Target;
  onSelectionChange?: (selection: DiscoverySelection) => void;
};

type EndpointRow = EndpointDto & { selected: boolean };

const HTTP_METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE"] as const;

function toRows(endpoints: EndpointDto[], selectedIds: Set<string>): EndpointRow[] {
  return endpoints.map((endpoint) => ({
    ...endpoint,
    selected: selectedIds.has(endpoint.id),
  }));
}

export function DiscoveryStep({ target, onSelectionChange }: DiscoveryStepProps) {
  const { actions } = useAppStore();
  const { notify } = useToast();

  const [scanId, setScanId] = useState<string | null>(null);
  const [endpoints, setEndpoints] = useState<EndpointDto[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [stats, setStats] = useState<DiscoveryStatsDto | null>(null);
  const [running, setRunning] = useState(false);
  const [completed, setCompleted] = useState(false);
  const [activePhaseIndex, setActivePhaseIndex] = useState(0);
  const [formError, setFormError] = useState<string | null>(null);

  const [manualMethod, setManualMethod] = useState<(typeof HTTP_METHODS)[number]>("GET");
  const [manualPath, setManualPath] = useState("");
  const [addingManual, setAddingManual] = useState(false);

  useEffect(() => {
    if (!running || completed) return;
    setActivePhaseIndex(0);
    const timer = setInterval(() => {
      setActivePhaseIndex((index) => (index + 1) % DISCOVERY_PHASES.length);
    }, 1200);
    return () => clearInterval(timer);
  }, [running, completed]);

  useEffect(() => {
    onSelectionChange?.({
      scanId,
      selectedCount: selectedIds.size,
      selectedEndpointIds: [...selectedIds],
    });
  }, [scanId, selectedIds, onSelectionChange]);

  const statuses = phaseStatuses(running, completed, activePhaseIndex);

  async function handleStartDiscovery() {
    if (running) return;
    setRunning(true);
    setCompleted(false);
    setFormError(null);
    setStats(null);

    try {
      const result = await actions.runDiscovery(target.id);
      setScanId(result.scan.id);
      setEndpoints(result.endpoints);
      setStats(result.stats);
      setSelectedIds(new Set(result.endpoints.map((e) => e.id)));
      setCompleted(true);
      notify(
        `Discovery complete — ${result.stats.endpoint_count} endpoint(s) found`,
        "success",
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : "Discovery failed";
      setFormError(message);
      notify(message, "error");
      setCompleted(false);
    } finally {
      setRunning(false);
    }
  }

  async function handleAddManual(e: React.FormEvent) {
    e.preventDefault();
    if (!scanId) {
      setFormError("Run discovery first to create a scan for this target.");
      return;
    }
    const path = manualPath.trim();
    if (!path.startsWith("/")) {
      setFormError("Path must start with / (e.g. /v1/chat).");
      return;
    }

    setAddingManual(true);
    setFormError(null);
    try {
      const created = await createEndpoint(scanId, target.id, manualMethod, path);
      await actions.refresh();
      const rows = await listEndpoints(scanId);
      setEndpoints(rows);
      setSelectedIds((prev) => new Set([...prev, created.id]));
      setManualPath("");
      notify("Manual endpoint added", "success");
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to add endpoint";
      setFormError(message);
      notify(message, "error");
    } finally {
      setAddingManual(false);
    }
  }

  function toggleEndpoint(id: string) {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleAll(checked: boolean) {
    if (checked) {
      setSelectedIds(new Set(endpoints.map((e) => e.id)));
    } else {
      setSelectedIds(new Set());
    }
  }

  const rows = toRows(endpoints, selectedIds);
  const allSelected = endpoints.length > 0 && selectedIds.size === endpoints.length;

  const columns = [
    {
      key: "selected",
      header: " ",
      width: "44px",
      render: (row: EndpointRow) => (
        <input
          type="checkbox"
          checked={row.selected}
          onChange={() => toggleEndpoint(row.id)}
          aria-label={`Select ${row.url}`}
        />
      ),
    },
    {
      key: "method",
      header: "Method",
      width: "90px",
      render: (row: EndpointRow) => row.method ?? "—",
    },
    {
      key: "url",
      header: "Endpoint",
      render: (row: EndpointRow) => (
        <div>
          <span className="mono text-sm">{row.url}</span>
          {row.evidence && <div className="text-muted text-sm">{row.evidence}</div>}
        </div>
      ),
    },
    {
      key: "confidence",
      header: "Confidence",
      width: "100px",
      render: (row: EndpointRow) => `${Math.round(row.confidence * 100)}%`,
    },
    {
      key: "source",
      header: "Source",
      width: "110px",
      render: (row: EndpointRow) => {
        const label = endpointSourceLabel(row.kind, row.source_url);
        return (
          <Badge variant={label === "Manual" ? "info" : "muted"}>{label}</Badge>
        );
      },
    },
  ];

  return (
    <div className="wizard-step">
      <div className="wizard-discovery-phases">
        {DISCOVERY_PHASES.map((phase, index) => (
          <span
            key={phase.id}
            className={`wizard-discovery-phase wizard-discovery-phase--${statuses[index]}`}
          >
            {phase.label}
          </span>
        ))}
      </div>

      {(running || completed) && (
        <ProgressBar
          value={completed ? 100 : ((activePhaseIndex + 1) / DISCOVERY_PHASES.length) * 100}
          label={running ? "Discovery running…" : "Discovery complete"}
          size="sm"
        />
      )}

      {stats && (
        <p className="text-muted text-sm wizard-discovery-stats">
          {stats.pages_fetched} pages · {stats.probes_sent} probes · {stats.endpoint_count}{" "}
          endpoints · {stats.duration_ms}ms
        </p>
      )}

      <div className="wizard-discovery-actions">
        <Button
          variant="primary"
          onClick={() => void handleStartDiscovery()}
          disabled={running}
        >
          {running ? "Discovering…" : completed ? "Re-run Discovery" : "Start Discovery"}
        </Button>
        {scanId && (
          <span className="text-muted text-sm">Scan ID: {scanId}</span>
        )}
      </div>

      {formError && <p className="text-danger">{formError}</p>}

      <div className="wizard-endpoints">
        <div className="wizard-endpoints__header">
          <h4 className="wizard-endpoints__title">Endpoints</h4>
          <div className="wizard-endpoints__meta">
            <label className="wizard-endpoints__select-all">
              <input
                type="checkbox"
                checked={allSelected}
                onChange={(e) => toggleAll(e.target.checked)}
                disabled={endpoints.length === 0}
              />
              <span>Select all</span>
            </label>
            <span className="text-muted text-sm">
              {selectedIds.size} of {endpoints.length} selected
            </span>
          </div>
        </div>
        <DataTable
          columns={columns}
          rows={rows}
          keyField="id"
          emptyMessage={
            completed
              ? "No endpoints discovered. Add manual endpoints below."
              : "Run discovery to populate endpoints from the target."
          }
        />
      </div>

      <div className="wizard-manual-endpoints">
        <h4 className="wizard-endpoints__title">Manual endpoints</h4>
        <form className="wizard-manual-form" onSubmit={handleAddManual}>
          <label className="field">
            <span className="field__label">Method</span>
            <select
              className="input"
              value={manualMethod}
              onChange={(e) =>
                setManualMethod(e.target.value as (typeof HTTP_METHODS)[number])
              }
              disabled={!scanId || addingManual}
            >
              {HTTP_METHODS.map((method) => (
                <option key={method} value={method}>
                  {method}
                </option>
              ))}
            </select>
          </label>
          <label className="field wizard-manual-form__path">
            <span className="field__label">Path</span>
            <input
              className="input"
              placeholder="/v1/chat/completions"
              value={manualPath}
              onChange={(e) => setManualPath(e.target.value)}
              disabled={!scanId || addingManual}
            />
          </label>
          <Button
            variant="secondary"
            type="submit"
            disabled={!scanId || addingManual || !manualPath.trim()}
          >
            {addingManual ? "Adding…" : "Add Endpoint"}
          </Button>
        </form>
        {!scanId && (
          <p className="text-muted text-sm">Start discovery to enable manual endpoints.</p>
        )}
      </div>
    </div>
  );
}
