import { Badge, Button, ProgressBar, StatusBadge } from "@/shared/components";
import { formatScanProgressPercent } from "@/features/scans/scanProgressFormat";
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
    const tests = `${status.testcases_completed ?? 0}/${status.testcases_total ?? "—"}`;
    return `${formatScanProgressPercent(status.progress_percent)} · ${tests} active tests`;
  }
  return "Scan in progress";
}

function scanStartedLabel(scan: ScanRun): string {
  return `Started: ${formatTimestamp(scan.startedAt ?? scan.createdAt)}`;
}

function scanDurationLabel(scan: ScanRun): string {
  if (scan.startedAt && scan.completedAt) {
    return formatDurationMs(
      new Date(scan.completedAt).getTime() - new Date(scan.startedAt).getTime(),
    );
  }
  return "—";
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
        {status.agent_mode && <Badge variant="info">Agent</Badge>}
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
          <dd>{formatScanProgressPercent(status.progress_percent)}</dd>
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
        {status.agent_mode && (
          <div className="scan-monitor-card__metric--wide">
            <dt>Agent phase</dt>
            <dd>
              {status.current_phase ?? "—"}
              {status.current_attempt != null ? ` · attempt ${status.current_attempt}` : ""}
              {status.current_retry != null && status.current_retry > 0
                ? ` · retry ${status.current_retry}`
                : ""}
            </dd>
          </div>
        )}
      </dl>

      {isActive && (
        <div
          className="card-footer"
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
        >
          <span className="card-footer-meta text-sm text-muted">{scanStartedLabel(scan)}</span>
          <div className="card-footer-actions scan-monitor-card__controls">
            {canPause && (
              <Button
                variant="secondary"
                size="sm"
                disabled={controlPending || status.pause_pending === true}
                onClick={onPause}
              >
                {status.pause_pending ? "Pausing…" : "Pause"}
              </Button>
            )}
            {canResume && (
              <Button
                variant="secondary"
                size="sm"
                disabled={controlPending}
                onClick={onResume}
              >
                Resume
              </Button>
            )}
            <Button variant="danger" size="sm" disabled={controlPending} onClick={onStop}>
              Stop
            </Button>
          </div>
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
          <dt>Duration</dt>
          <dd className="text-sm">{scanDurationLabel(scan)}</dd>
        </div>
      </dl>
      <div className="card-footer">
        <span className="card-footer-meta text-sm text-muted">{scanStartedLabel(scan)}</span>
      </div>
    </div>
  );
}
