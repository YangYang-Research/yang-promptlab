import { Button, ProgressBar, StatusBadge } from "@/shared/components";
import type { ScanStatusDto } from "@/shared/ipc";
import type { ScanRun } from "@/shared/types";

type ScanMonitorCardProps = {
  scan: ScanRun;
  status: ScanStatusDto;
  projectName: string;
  targetName: string;
  controlPending: boolean;
  onPause: () => void;
  onResume: () => void;
  onStop: () => void;
};

function progressLabel(status: ScanStatusDto): string {
  if (status.status === "paused") {
    return "Scan paused";
  }
  if (status.total > 0) {
    return `${status.completed} / ${status.total} tests`;
  }
  return "Scan in progress";
}

export function ScanMonitorCard({
  scan,
  status,
  projectName,
  targetName,
  controlPending,
  onPause,
  onResume,
  onStop,
}: ScanMonitorCardProps) {
  const isActive = status.status === "running" || status.status === "paused";
  const canPause = status.status === "running";
  const canResume = status.status === "paused";

  return (
    <div className="scan-monitor-card">
      <div className="scan-monitor-card__header">
        <div>
          <h3 className="scan-monitor-card__title">{scan.name}</h3>
          <p className="text-muted text-sm">
            {projectName} · {targetName}
          </p>
          <p className="text-muted text-sm mono">{scan.id}</p>
        </div>
        <StatusBadge status={status.status as ScanRun["status"]} />
      </div>

      {isActive && (
        <ProgressBar
          value={status.progress_percent}
          label={progressLabel(status)}
          size="sm"
        />
      )}

      <dl className="scan-monitor-card__metrics">
        <div>
          <dt>Findings</dt>
          <dd>{status.findings_count}</dd>
        </div>
        <div>
          <dt>Progress</dt>
          <dd>{Math.round(status.progress_percent)}%</dd>
        </div>
        <div className="scan-monitor-card__metric--wide">
          <dt>Current endpoint</dt>
          <dd className="mono text-sm">
            {status.current_endpoint ?? (isActive ? "Starting…" : "—")}
          </dd>
        </div>
        <div className="scan-monitor-card__metric--wide">
          <dt>Current test</dt>
          <dd>{status.current_test ?? (isActive ? "—" : "—")}</dd>
        </div>
      </dl>

      {isActive && (
        <div className="scan-monitor-card__controls">
          <Button
            variant="secondary"
            size="sm"
            disabled={controlPending || !canPause}
            onClick={onPause}
          >
            Pause
          </Button>
          <Button
            variant="secondary"
            size="sm"
            disabled={controlPending || !canResume}
            onClick={onResume}
          >
            Resume
          </Button>
          <Button
            variant="danger"
            size="sm"
            disabled={controlPending}
            onClick={onStop}
          >
            Stop
          </Button>
        </div>
      )}
    </div>
  );
}

type ScanHistoryCardProps = {
  scan: ScanRun;
  findingsCount: number;
  projectName: string;
  targetName: string;
};

export function ScanHistoryCard({
  scan,
  findingsCount,
  projectName,
  targetName,
}: ScanHistoryCardProps) {
  return (
    <div className="scan-monitor-card scan-monitor-card--history">
      <div className="scan-monitor-card__header">
        <div>
          <h3 className="scan-monitor-card__title">{scan.name}</h3>
          <p className="text-muted text-sm">
            {projectName} · {targetName}
          </p>
        </div>
        <StatusBadge status={scan.status} />
      </div>
      <dl className="scan-monitor-card__metrics">
        <div>
          <dt>Findings</dt>
          <dd>{findingsCount}</dd>
        </div>
        <div>
          <dt>Started</dt>
          <dd className="text-sm">
            {scan.startedAt
              ? new Date(scan.startedAt).toLocaleString()
              : new Date(scan.createdAt).toLocaleString()}
          </dd>
        </div>
      </dl>
    </div>
  );
}
