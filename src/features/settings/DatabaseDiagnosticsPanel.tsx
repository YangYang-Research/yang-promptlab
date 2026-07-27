import { Badge } from "@/shared/components";
import { formatBytes, shortenPromptLabPath } from "@/shared/utils/format";
import type { DbHealthDto } from "@/shared/ipc/environment";

type DatabaseDiagnosticsPanelProps = {
  health: DbHealthDto;
  root?: string | null;
};

export function DatabaseDiagnosticsPanel({ health, root }: DatabaseDiagnosticsPanelProps) {
  const pathLabel = root
    ? shortenPromptLabPath(health.path, root)
    : health.path;

  return (
    <div className="database-diagnostics">
      <div className="database-diagnostics__header">
        <p className="text-muted text-sm database-diagnostics__lead">
          SQLite connectivity for the local PromptLab database.
        </p>
        <Badge variant={health.connected ? "success" : "danger"}>
          {health.connected ? "Connected" : "Disconnected"}
        </Badge>
      </div>
      <dl className="database-diagnostics__meta">
        <div className="database-diagnostics__row">
          <dt>Path</dt>
          <dd className="mono">{pathLabel}</dd>
        </div>
        <div className="database-diagnostics__row">
          <dt>Size</dt>
          <dd>{formatBytes(health.sizeBytes)}</dd>
        </div>
      </dl>
    </div>
  );
}
