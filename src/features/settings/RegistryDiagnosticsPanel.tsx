import { StatusBadge } from "@/shared/components";
import type { ModelRegistryDiagnosticsDto } from "@/shared/ipc/models";

type RegistryDiagnosticsPanelProps = {
  diagnostics: ModelRegistryDiagnosticsDto;
};

export function RegistryDiagnosticsPanel({ diagnostics }: RegistryDiagnosticsPanelProps) {
  return (
    <div className="registry-diagnostics">
      <p className="text-muted text-sm">
        Startup validation for <code>resources/models.json</code> (GGUF-first schema).
      </p>
      <div className="model-catalog__row">
        <div>
          <p className="text-sm">
            <strong>Total:</strong> {diagnostics.totalModels}
          </p>
          <p className="text-sm">
            <strong>Valid:</strong> {diagnostics.validModels}
          </p>
          <p className="text-sm">
            <strong>Invalid:</strong> {diagnostics.invalidModels}
          </p>
        </div>
        <StatusBadge status={diagnostics.healthy ? "completed" : "failed"} />
      </div>
      {diagnostics.issues.length > 0 && (
        <div className="model-catalog">
          {diagnostics.issues.map((issue, index) => (
            <div key={`${issue.id}-${issue.field}-${index}`} className="model-catalog__row">
              <div>
                <strong className="mono">{issue.id || "(missing id)"}</strong>
                <p className="text-muted text-sm">
                  {issue.field}: {issue.message}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
