import { Button, Card } from "@/shared/components";
import type { ModelDownloadProgressDto } from "@/shared/ipc/models";
import { formatBytes, formatEta, formatSpeed } from "@/shared/utils/format";

type DownloadManagerCardProps = {
  progress: ModelDownloadProgressDto;
  backendConnected: boolean;
  onPause: () => void;
  onResume: () => void;
  onCancel: () => void;
  onRetryVerify?: () => void;
  onCancelVerify?: () => void;
  onStartVerify?: () => void;
  onDismiss?: () => void;
};

function statusLabel(status: string): string {
  switch (status) {
    case "downloading":
      return "Downloading";
    case "paused":
      return "Paused";
    case "pending":
      return "Pending";
    case "verifying":
      return "Verifying";
    case "downloaded":
      return "Downloaded";
    case "verify_failed":
      return "Verify failed";
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
  onRetryVerify,
  onCancelVerify,
  onStartVerify,
  onDismiss,
}: DownloadManagerCardProps) {
  const percent = progress.percent ?? 0;
  const remaining =
    progress.remainingBytes ??
    (progress.totalBytes != null
      ? Math.max(0, progress.totalBytes - progress.downloadedBytes)
      : null);
  const isPaused = progress.status === "paused";
  const isVerifying = progress.status === "verifying";
  const isDownloaded = progress.status === "downloaded";
  const isVerifyFailed = progress.status === "verify_failed";
  const isActive =
    progress.status === "downloading" ||
    progress.status === "paused" ||
    progress.status === "pending";

  return (
    <Card className="model-card model-card--wide download-manager">
      <div className="download-manager__header">
        <div>
          <h3 className="card__title">Download Manager</h3>
          <p className="text-muted text-sm">
            {progress.catalogId}
            {progress.resumed ? " · resumed" : ""}
            {isVerifying ? " · checking SHA256 integrity" : ""}
            {isDownloaded ? " · ready to verify" : ""}
            {isVerifyFailed ? " · file kept on disk" : ""}
          </p>
        </div>
        <div className="download-manager__header-actions">
          {isActive && !isVerifying && (
            <>
              {!isPaused ? (
                <Button variant="ghost" disabled={!backendConnected} onClick={onPause}>
                  Pause
                </Button>
              ) : (
                <Button variant="ghost" disabled={!backendConnected} onClick={onResume}>
                  Resume
                </Button>
              )}
            </>
          )}
          {isVerifyFailed && (
            <>
              <Button
                variant="secondary"
                disabled={!backendConnected}
                onClick={onRetryVerify}
              >
                Retry verify
              </Button>
              <Button variant="ghost" disabled={!backendConnected} onClick={onDismiss ?? onCancel}>
                Dismiss
              </Button>
            </>
          )}
          {progress.status === "failed" && (
            <Button variant="ghost" disabled={!backendConnected} onClick={onDismiss ?? onCancel}>
              Dismiss
            </Button>
          )}
          <span className={`download-manager__status download-manager__status--${progress.status}`}>
            {statusLabel(progress.status)}
          </span>
        </div>
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
            {isVerifying
              ? "Verifying…"
              : isDownloaded
                ? "Ready"
                : isVerifyFailed
                  ? "Download complete"
                  : progress.percent != null
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
          <strong>
            {isVerifying || isVerifyFailed || isDownloaded
              ? "—"
              : remaining != null
                ? formatBytes(remaining)
                : "—"}
          </strong>
        </div>
        <div>
          <span className="download-manager__stat-label">Speed</span>
          <strong>
            {isVerifying || isVerifyFailed || isDownloaded
              ? "—"
              : formatSpeed(progress.speedBytesPerSec)}
          </strong>
        </div>
        <div>
          <span className="download-manager__stat-label">ETA</span>
          <strong>
            {isVerifying || isVerifyFailed || isDownloaded
              ? "—"
              : formatEta(progress.etaSeconds)}
          </strong>
        </div>
      </div>

      {(progress.status === "failed" || isVerifyFailed) && (
        <p className="text-danger download-manager__error">
          {progress.error ?? (isVerifyFailed ? "Verification failed." : "Download failed.")}
        </p>
      )}

      {isActive && !isVerifying && (
        <div className="download-manager__footer">
          <Button variant="danger" disabled={!backendConnected} onClick={onCancel}>
            Cancel
          </Button>
        </div>
      )}

      {(isDownloaded || isVerifying) && (
        <div className="download-manager__footer">
          {isVerifying ? (
            <Button variant="danger" disabled={!backendConnected} onClick={onCancelVerify}>
              Cancel
            </Button>
          ) : (
            <Button variant="primary" disabled={!backendConnected} onClick={onStartVerify}>
              Verify
            </Button>
          )}
        </div>
      )}
    </Card>
  );
}
