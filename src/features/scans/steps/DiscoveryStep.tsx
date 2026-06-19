import { useEffect, useState } from "react";

import { useAppStore } from "@/app/store/AppStore";
import {
  Badge,
  Button,
  DataTable,
  ProgressBar,
  Select,
} from "@/shared/components";
import type { EndpointDto } from "@/shared/ipc";
import { createEndpoint, listEndpoints, updateEndpoint } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import type { Target } from "@/shared/types";

import {
  DISCOVERY_PHASES,
  endpointSourceLabel,
  phaseStatuses,
} from "../discoveryPhases";
import {
  aggregatePlatformSummary,
  platformLabel,
} from "../fingerprintPlan";
import {
  confidenceLabel,
  endpointPath,
  HTTP_METHODS,
  inferEndpointMethod,
} from "../endpointMethod";
import type { DiscoveryWizardState } from "../wizardState";

export type DiscoverySelection = {
  scanId: string | null;
  selectedCount: number;
  selectedEndpointIds: string[];
};

type DiscoveryStepProps = {
  target: Target;
  discovery: DiscoveryWizardState;
  onDiscoveryChange: (patch: Partial<DiscoveryWizardState>) => void;
};

type EndpointRow = EndpointDto & { selected: boolean };

function toRows(endpoints: EndpointDto[], selectedIds: string[]): EndpointRow[] {
  const selected = new Set(selectedIds);
  return endpoints.map((endpoint) => ({
    ...endpoint,
    selected: selected.has(endpoint.id),
  }));
}

function displayMethod(endpoint: EndpointDto): string {
  return endpoint.method ?? inferEndpointMethod(endpoint.url);
}

export function DiscoveryStep({ target, discovery, onDiscoveryChange }: DiscoveryStepProps) {
  const { actions } = useAppStore();
  const { notify } = useToast();

  const [endpoints, setEndpoints] = useState<EndpointDto[]>([]);
  const [running, setRunning] = useState(false);
  const [activePhaseIndex, setActivePhaseIndex] = useState(0);
  const [formError, setFormError] = useState<string | null>(null);
  const [addingManual, setAddingManual] = useState(false);
  const [loadingEndpoints, setLoadingEndpoints] = useState(false);
  const [updatingMethodId, setUpdatingMethodId] = useState<string | null>(null);

  const { scanId, selectedEndpointIds, completed, stats, manualMethod, manualPath, endpoints: cachedEndpoints } =
    discovery;

  useEffect(() => {
    if (cachedEndpoints.length > 0) {
      setEndpoints(cachedEndpoints);
    }
  }, [cachedEndpoints]);

  useEffect(() => {
    if (!running || completed) return;
    setActivePhaseIndex(0);
    const timer = setInterval(() => {
      setActivePhaseIndex((index) => (index + 1) % DISCOVERY_PHASES.length);
    }, 1200);
    return () => clearInterval(timer);
  }, [running, completed]);

  useEffect(() => {
    if (!scanId || !completed) {
      setEndpoints([]);
      return;
    }

    let cancelled = false;
    setLoadingEndpoints(true);
    void listEndpoints(scanId)
      .then((rows) => {
        if (cancelled) return;
        setEndpoints(rows);
        onDiscoveryChange({ endpoints: rows });
      })
      .catch(() => {
        if (!cancelled) setEndpoints([]);
      })
      .finally(() => {
        if (!cancelled) setLoadingEndpoints(false);
      });

    return () => {
      cancelled = true;
    };
  }, [scanId, completed]);

  const statuses = phaseStatuses(running, completed, activePhaseIndex);

  async function handleStartDiscovery() {
    if (running) return;
    setRunning(true);
    setFormError(null);
    const previousSelection = new Set(selectedEndpointIds);
    onDiscoveryChange({
      completed: false,
      stats: null,
    });

    try {
      const result = await actions.runDiscovery(target.id, scanId);
      const resultIds = new Set(result.endpoints.map((endpoint) => endpoint.id));
      const preserved = [...previousSelection].filter((id) => resultIds.has(id));
      const newlyDiscovered = result.endpoints
        .map((endpoint) => endpoint.id)
        .filter((id) => !previousSelection.has(id));
      const nextSelected = [...preserved, ...newlyDiscovered];

      setEndpoints(result.endpoints);
      onDiscoveryChange({
        scanId: result.scan.id,
        completed: true,
        stats: result.stats,
        endpoints: result.endpoints,
        selectedEndpointIds: nextSelected.length > 0 ? nextSelected : result.endpoints.map((e) => e.id),
      });
      notify(
        `Discovery complete — ${result.stats.endpoint_count} endpoint(s) found`,
        "success",
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : "Discovery failed";
      setFormError(message);
      notify(message, "error");
      onDiscoveryChange({ completed: false });
    } finally {
      setRunning(false);
    }
  }

  async function handleMethodChange(endpointId: string, method: string) {
    setUpdatingMethodId(endpointId);
    try {
      const updated = await updateEndpoint(endpointId, method);
      const rows = endpoints.map((row) => (row.id === endpointId ? updated : row));
      setEndpoints(rows);
      onDiscoveryChange({ endpoints: rows });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Failed to update method";
      notify(message, "error");
    } finally {
      setUpdatingMethodId(null);
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
    if (!manualMethod.trim()) {
      setFormError("HTTP method is required.");
      return;
    }

    setAddingManual(true);
    setFormError(null);
    try {
      const created = await createEndpoint(scanId, target.id, manualMethod, path);
      await actions.refresh();
      const rows = await listEndpoints(scanId);
      setEndpoints(rows);
      onDiscoveryChange({
        selectedEndpointIds: [...selectedEndpointIds, created.id],
        endpoints: rows,
        manualPath: "",
      });
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
    const next = new Set(selectedEndpointIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onDiscoveryChange({ selectedEndpointIds: [...next] });
  }

  function toggleAll(checked: boolean) {
    onDiscoveryChange({
      selectedEndpointIds: checked ? endpoints.map((endpoint) => endpoint.id) : [],
    });
  }

  const rows = toRows(endpoints, selectedEndpointIds);
  const allSelected = endpoints.length > 0 && selectedEndpointIds.length === endpoints.length;
  const platformSummary = aggregatePlatformSummary(endpoints, selectedEndpointIds);

  const columns = [
    {
      key: "selected",
      header: "✓",
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
      key: "url",
      header: "Endpoint",
      render: (row: EndpointRow) => (
        <span className="mono text-sm">{endpointPath(row.url)}</span>
      ),
    },
    {
      key: "method",
      header: "Method",
      width: "110px",
      render: (row: EndpointRow) => (
        <Select
          className="scan-endpoint-method"
          value={displayMethod(row)}
          disabled={updatingMethodId === row.id}
          onChange={(e) => void handleMethodChange(row.id, e.target.value)}
          aria-label={`HTTP method for ${row.url}`}
        >
          {HTTP_METHODS.map((method) => (
            <option key={method} value={method}>
              {method}
            </option>
          ))}
        </Select>
      ),
    },
    {
      key: "source",
      header: "Source",
      width: "120px",
      render: (row: EndpointRow) => {
        const label = endpointSourceLabel(row.kind, row.source_url);
        return (
          <Badge variant={label === "Manual" ? "info" : "muted"}>{label}</Badge>
        );
      },
    },
    {
      key: "confidence",
      header: "Confidence",
      width: "110px",
      render: (row: EndpointRow) => confidenceLabel(row.confidence),
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

      {completed && stats && (
        <p className="text-muted text-sm wizard-discovery-stats">
          {stats.pages_fetched} pages · {stats.probes_sent} probes · {stats.endpoint_count}{" "}
          endpoints · {stats.duration_ms}ms
          {platformSummary.length > 0 && (
            <>
              {" "}
              · Detected:{" "}
              {platformSummary.map((p) => platformLabel(p.platform)).join(", ")}
            </>
          )}
        </p>
      )}

      {!completed && !running && (
        <p className="text-muted text-sm">
          Start discovery to enumerate endpoints from {target.url}.
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
        {scanId && completed && (
          <span className="text-muted text-sm">Scan ID: {scanId}</span>
        )}
      </div>

      {formError && <p className="text-danger">{formError}</p>}

      {completed && (
        <>
          <div className="wizard-endpoints">
            <div className="wizard-endpoints__header">
              <h4 className="wizard-endpoints__title">Discovered endpoints</h4>
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
                  {selectedEndpointIds.length} of {endpoints.length} selected
                </span>
              </div>
            </div>
            <DataTable
              columns={columns}
              rows={rows}
              keyField="id"
              emptyMessage={
                loadingEndpoints
                  ? "Loading endpoints…"
                  : "No endpoints discovered. Add manual endpoints below."
              }
            />
          </div>

          <div className="wizard-manual-endpoints">
            <h4 className="wizard-endpoints__title">Manual endpoints</h4>
            <form className="wizard-manual-form" onSubmit={handleAddManual}>
              <label className="field">
                <span className="field__label">Endpoint</span>
                <input
                  className="input"
                  placeholder="/v1/chat/completions"
                  value={manualPath}
                  onChange={(e) => {
                    const path = e.target.value;
                    onDiscoveryChange({
                      manualPath: path,
                      manualMethod:
                        manualMethod || inferEndpointMethod(path),
                    });
                  }}
                  disabled={!scanId || addingManual}
                  required
                />
              </label>
              <label className="field">
                <span className="field__label">Method</span>
                <Select
                  value={manualMethod}
                  onChange={(e) => onDiscoveryChange({ manualMethod: e.target.value })}
                  disabled={!scanId || addingManual}
                  required
                >
                  {HTTP_METHODS.map((method) => (
                    <option key={method} value={method}>
                      {method}
                    </option>
                  ))}
                </Select>
              </label>
              <Button
                variant="secondary"
                type="submit"
                disabled={!scanId || addingManual || !manualPath.trim() || !manualMethod.trim()}
              >
                {addingManual ? "Adding…" : "Add Endpoint"}
              </Button>
            </form>
          </div>
        </>
      )}
    </div>
  );
}
