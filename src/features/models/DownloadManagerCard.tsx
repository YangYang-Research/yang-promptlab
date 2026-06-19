import { Button, Card } from "@/shared/components";
import type { ModelDownloadProgressDto } from "@/shared/ipc/models";
import { formatBytes, formatEta, formatSpeed } from "@/shared/utils/format";

type DownloadManagerCardProps = {
  progress: ModelDownloadProgressDto;
  backendConnected: boolean;
  onPause: () => void;
  onResume: () => void;
  onCancel: () => void;
};

function statusLabel(status: string): string {
  switch (status) {
    case "downloading":
      return "Downloading";
    case "paused":
      return "Paused";
    case "pending":
      return "Pending";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    default:
      return status;
  }
}

export function DownloadManagerCard({
  progress,
  backendConnected,
  onPause,
  onResume,
  onCancel,
}: DownloadManagerCardProps) {
  const percent = progress.percent ?? 0;
  const remaining =
    progress.remainingBytes ??
    (progress.totalBytes != null
      ? Math.max(0, progress.totalBytes - progress.downloadedBytes)
      : null);
  const isPaused = progress.status === "paused";
  const isActive =
    progress.status === "downloading" || progress.status === "paused" || progress.status === "pending";

  return (
    <Card className="model-card model-card--wide download-manager">
      <div className="download-manager__header">
        <div>
          <h3 className="card__title">Download Manager</h3>
          <p className="text-muted text-sm">
            {progress.catalogId}
            {progress.resumed ? " · resumed" : ""}
          </p>
        </div>
        <span className={`download-manager__status download-manager__status--${progress.status}`}>
          {statusLabel(progress.status)}
        </span>
      </div>

      <div
        className="download-manager__bar"
        role="progressbar"
        aria-valuenow={percent}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={`Download progress for ${progress.catalogId}`}
      >
        <div className="download-manager__bar-fill" style={{ width: `${Math.min(100, percent)}%` }} />
      </div>

      <div className="download-manager__stats">
        <div>
          <span className="download-manager__stat-label">Progress</span>
          <strong>
            {progress.percent != null
              ? `${progress.percent.toFixed(1)}%`
              : formatBytes(progress.downloadedBytes)}
          </strong>
        </div>
        <div>
          <span className="download-manager__stat-label">Downloaded</span>
          <strong>{formatBytes(progress.downloadedBytes)}</strong>
          {progress.totalBytes != null ? (
            <span className="text-muted text-sm"> / {formatBytes(progress.totalBytes)}</span>
          ) : null}
        </div>
        <div>
          <span className="download-manager__stat-label">Remaining</span>
          <strong>{remaining != null ? formatBytes(remaining) : "—"}</strong>
        </div>
        <div>
          <span className="download-manager__stat-label">Speed</span>
          <strong>{formatSpeed(progress.speedBytesPerSec)}</strong>
        </div>
        <div>
          <span className="download-manager__stat-label">ETA</span>
          <strong>{formatEta(progress.etaSeconds)}</strong>
        </div>
      </div>

      {progress.status === "failed" && (
        <div className="download-manager__error">
          <p className="text-danger">{progress.error ?? "Download failed."}</p>
          <div className="model-card__actions">
            <Button variant="ghost" disabled={!backendConnected} onClick={onCancel}>
              Dismiss
            </Button>
          </div>
        </div>
      )}

      {isActive && (
        <div className="model-card__actions">
          <Button
            variant="ghost"
            disabled={!backendConnected || isPaused}
            onClick={onPause}
          >
            Pause
          </Button>
          <Button
            variant="ghost"
            disabled={!backendConnected || !isPaused}
            onClick={onResume}
          >
            Resume
          </Button>
          <Button variant="ghost" disabled={!backendConnected} onClick={onCancel}>
            Cancel
          </Button>
        </div>
      )}
    </Card>
  );
}
