import { Badge } from "@/shared/components";
import type { VerificationConsoleEntryDto } from "../targetProfile";

type VerificationConsoleProps = {
  entry: VerificationConsoleEntryDto | null;
  pending?: boolean;
};

export function VerificationConsole({ entry, pending = false }: VerificationConsoleProps) {
  if (!entry) {
    return (
      <div className="verification-console verification-console--empty">
        <p className="text-muted">Run verification to inspect the outgoing request and response.</p>
      </div>
    );
  }

  const statusLabel =
    entry.statusCode > 0 ? `${entry.method} ${entry.statusCode}` : entry.method || "REQUEST";

  return (
    <div className="verification-console">
      <div className="verification-console__header">
        <Badge variant={pending ? "warning" : entry.success ? "success" : "danger"}>
          {pending ? "Sending…" : entry.success ? "Verified" : "Failed"}
        </Badge>
        <span className="text-muted">
          {statusLabel}
          {entry.responseTimeMs > 0 ? ` · ${entry.responseTimeMs}ms` : ""}
        </span>
      </div>

      {entry.message && <p className="verification-console__message">{entry.message}</p>}

      {entry.requestLog && (
        <details open>
          <summary>Full request (curl)</summary>
          <pre className="verification-console__block verification-console__block--log">
            {entry.requestLog}
          </pre>
        </details>
      )}

      {entry.authDebug && (
        <details open>
          <summary>Auth debug</summary>
          <pre className="verification-console__block">{entry.authDebug}</pre>
        </details>
      )}

      {entry.responsePreview && (
        <details open>
          <summary>Response (from target)</summary>
          <pre className="verification-console__block verification-console__block--log">
            {entry.responsePreview}
          </pre>
        </details>
      )}

      {entry.statusCode > 0 && (
        <p className="text-muted text-sm verification-console__hint">
          Backend returned HTTP {entry.statusCode}. Compare with the curl block above if results
          differ.
        </p>
      )}
    </div>
  );
}
