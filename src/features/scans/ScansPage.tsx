import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import {
  Button,
  Card,
  EmptyState,
  PageHeader,
} from "@/shared/components";
import { pauseScan, resumeScan, stopScan } from "@/shared/ipc";
import { useToast } from "@/shared/notifications";
import type { ScanRun } from "@/shared/types";

import { ScanHistoryCard, ScanMonitorCard } from "./ScanMonitorCard";
import { mergeScanStatus, useScanStatuses } from "./useScanStatuses";

function isActiveScan(scan: ScanRun): boolean {
  return scan.status === "running" || scan.status === "paused" || scan.status === "pending";
}

export function ScansPage() {
  const { scans, targets, projects, findings, loading, error, actions } = useAppStore();
  const { notify } = useToast();
  const [controlPending, setControlPending] = useState<string | null>(null);

  const findingsByScan = useMemo(() => {
    const map = new Map<string, number>();
    for (const finding of findings) {
      map.set(finding.scanId, (map.get(finding.scanId) ?? 0) + 1);
    }
    return map;
  }, [findings]);

  const sortedScans = useMemo(
    () => [...scans].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [scans],
  );

  const activeScans = useMemo(
    () => sortedScans.filter(isActiveScan),
    [sortedScans],
  );

  const historyScans = useMemo(
    () => sortedScans.filter((scan) => !isActiveScan(scan)),
    [sortedScans],
  );

  const activeScanIds = useMemo(() => activeScans.map((scan) => scan.id), [activeScans]);
  const liveStatuses = useScanStatuses(activeScanIds, activeScanIds.length > 0);

  const targetName = (targetId: string | null) =>
    targetId ? targets.find((t) => t.id === targetId)?.name ?? "—" : "—";

  const projectName = (projectId: string) =>
    projects.find((p) => p.id === projectId)?.name ?? "—";

  const runControl = useCallback(
    async (scanId: string, action: "pause" | "resume" | "stop") => {
      setControlPending(scanId);
      try {
        if (action === "pause") {
          await pauseScan(scanId);
          notify("Scan paused", "info");
        } else if (action === "resume") {
          await resumeScan(scanId);
          notify("Scan resumed", "success");
        } else {
          await stopScan(scanId);
          notify("Scan stopped", "info");
        }
        await actions.refresh();
      } catch (err) {
        const message = err instanceof Error ? err.message : "Scan control failed";
        notify(message, "error");
      } finally {
        setControlPending(null);
      }
    },
    [actions, notify],
  );

  return (
    <div className="page">
      <PageHeader
        title="Scans"
        description="Monitor background security scans"
        actions={
          <div className="discovery-controls">
            <Button variant="secondary" onClick={() => void actions.refresh()} disabled={loading}>
              Refresh
            </Button>
            <Link to="/scans/new" className="btn btn--primary">
              New Scan
            </Link>
          </div>
        }
      />

      {error && (
        <Card>
          <p className="text-danger">{error}</p>
        </Card>
      )}

      {sortedScans.length === 0 && !loading ? (
        <EmptyState
          title="No scans yet"
          description="Configure a new scan in the wizard to start a background attack job."
          action={
            <Link to="/scans/new" className="btn btn--primary">
              New Scan
            </Link>
          }
        />
      ) : (
        <>
          <section className="scan-monitor-section">
            <div className="scan-monitor-section__header">
              <h2 className="scan-monitor-section__title">Running scans</h2>
              <span className="text-muted text-sm">{activeScans.length} active</span>
            </div>

            {activeScans.length === 0 ? (
              <Card>
                <p className="text-muted">No scans are running right now.</p>
              </Card>
            ) : (
              <div className="scan-monitor-grid">
                {activeScans.map((scan) => {
                  const status = mergeScanStatus(
                    scan.id,
                    scan.status,
                    liveStatuses.get(scan.id),
                    findingsByScan.get(scan.id) ?? 0,
                  );

                  return (
                    <Card key={scan.id} className="scan-monitor-card-wrap">
                      <ScanMonitorCard
                        scan={scan}
                        status={status}
                        projectName={projectName(scan.projectId)}
                        targetName={targetName(scan.targetId)}
                        controlPending={controlPending === scan.id}
                        onPause={() => void runControl(scan.id, "pause")}
                        onResume={() => void runControl(scan.id, "resume")}
                        onStop={() => void runControl(scan.id, "stop")}
                      />
                    </Card>
                  );
                })}
              </div>
            )}
          </section>

          {historyScans.length > 0 && (
            <section className="scan-monitor-section">
              <div className="scan-monitor-section__header">
                <h2 className="scan-monitor-section__title">Recent scans</h2>
              </div>
              <div className="scan-monitor-grid">
                {historyScans.map((scan) => (
                  <Card key={scan.id} className="scan-monitor-card-wrap">
                    <ScanHistoryCard
                      scan={scan}
                      findingsCount={findingsByScan.get(scan.id) ?? 0}
                      projectName={projectName(scan.projectId)}
                      targetName={targetName(scan.targetId)}
                    />
                  </Card>
                ))}
              </div>
            </section>
          )}
        </>
      )}
    </div>
  );
}
