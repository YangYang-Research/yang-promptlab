import { Fragment } from "react";

import {
  formatVerificationLogTime,
  type VerificationLogLine,
} from "../verificationLog";

type VerificationConsoleProps = {
  lines: VerificationLogLine[];
  pending?: boolean;
};

export function VerificationConsole({ lines, pending = false }: VerificationConsoleProps) {
  return (
    <details className="verification-console" open={pending || lines.length > 0}>
      <summary className="verification-console__summary">
        Verification console
        {lines.length > 0 && (
          <span className="verification-console__count">{lines.length} line(s)</span>
        )}
      </summary>

      {lines.length === 0 ? (
        <p className="verification-console__empty text-muted">
          Run verification to inspect the outgoing request and response.
        </p>
      ) : (
        <pre className="verification-console__stream" aria-live="polite">
          {lines.map((line) => {
            const parts = line.message.split("\n");
            return (
              <span key={line.id} className="verification-console__line">
                <span className="verification-console__time">
                  {formatVerificationLogTime(line.timestamp)}
                </span>
                <span className="verification-console__text">
                  {parts.map((part, index) => (
                    <Fragment key={index}>
                      {index > 0 ? "\n" : null}
                      {index > 0 ? (
                        <span className="verification-console__continuation">{part}</span>
                      ) : (
                        part
                      )}
                    </Fragment>
                  ))}
                </span>
                {"\n"}
              </span>
            );
          })}
        </pre>
      )}
    </details>
  );
}
