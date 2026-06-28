import { Badge } from "@/shared/components";
import type { VerificationConsoleEntryDto } from "../targetProfile";

type VerificationConsoleProps = {
  entry: VerificationConsoleEntryDto | null;
};

export function VerificationConsole({ entry }: VerificationConsoleProps) {
  if (!entry) {
    return (
      <div className="verification-console verification-console--empty">
        <p className="text-muted">Run verification to inspect the outgoing request and response.</p>
      </div>
    );
  }

  return (
    <div className="verification-console">
      <div className="verification-console__header">
        <Badge variant={entry.success ? "success" : "danger"}>
          {entry.success ? "Verified" : "Failed"}
        </Badge>
        <span className="text-muted">
          {entry.method} {entry.statusCode} · {entry.responseTimeMs}ms
        </span>
      </div>
      <p className="verification-console__message">{entry.message}</p>
      <details open>
        <summary>Outgoing request</summary>
        <pre className="verification-console__block">{entry.url}</pre>
        <pre className="verification-console__block">{JSON.stringify(entry.headers, null, 2)}</pre>
        <pre className="verification-console__block">{entry.body}</pre>
      </details>
      {entry.responsePreview && (
        <details open>
          <summary>Response preview</summary>
          <pre className="verification-console__block">{entry.responsePreview}</pre>
        </details>
      )}
    </div>
  );
}
