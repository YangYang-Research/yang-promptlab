import { useEffect, useState } from "react";

import { Button, Card } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { getEnvironment, openRootDirectory, type EnvironmentStatusDto } from "@/shared/ipc/environment";
import { shortenPromptLabPath } from "@/shared/utils/format";

type EnvironmentsPanelProps = {
  backendConnected: boolean;
};

export function EnvironmentsPanel({ backendConnected }: EnvironmentsPanelProps) {
  const [status, setStatus] = useState<EnvironmentStatusDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!backendConnected) {
      setStatus(null);
      return;
    }
    setLoading(true);
    setError(null);
    void getEnvironment()
      .then(setStatus)
      .catch((err) => setError(toAppError(err).message))
      .finally(() => setLoading(false));
  }, [backendConnected]);

  async function handleOpenRoot() {
    setOpening(true);
    setError(null);
    try {
      await openRootDirectory();
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setOpening(false);
    }
  }

  return (
    <div className="settings-tab-panel">
      <Card>
        <h3 className="card__title">Environments</h3>
        <p className="text-muted text-sm">
          PromptLab stores all application data under a fixed root directory on this device.
        </p>
        {!backendConnected ? (
          <p className="text-muted text-sm">Connect to the desktop backend to view the root directory.</p>
        ) : loading ? (
          <p className="text-muted text-sm">Loading environment…</p>
        ) : status ? (
          <>
            <div className="settings-field">
              <label htmlFor="rootDir">Root Directory</label>
              <input
                id="rootDir"
                className="input mono"
                value={shortenPromptLabPath(status.root, status.root)}
                readOnly
                aria-readonly="true"
              />
            </div>
            <div className="settings-field__actions">
              <Button variant="secondary" disabled={opening} onClick={() => void handleOpenRoot()}>
                {opening ? "Opening…" : "Open Root Directory"}
              </Button>
            </div>
            {error ? <p className="text-danger text-sm">{error}</p> : null}
          </>
        ) : null}
      </Card>
    </div>
  );
}
