import { useEffect, useMemo, useState } from "react";

import { Button, Card } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { shortenPromptLabPath } from "@/shared/utils/format";
import {
  getEnvironment,
  getLogsFolderPath,
  getRecentLogEvents,
  listLogFiles,
  tailLogFile,
  type EnvironmentStatusDto,
  type LogFileInfoDto,
  type OcsfEventDto,
} from "@/shared/ipc/environment";
import { getModelsRegistryDiagnostics, type ModelRegistryDiagnosticsDto } from "@/shared/ipc/models";

import { RegistryDiagnosticsPanel } from "./RegistryDiagnosticsPanel";

type TroubleshootingPanelProps = {
  backendConnected: boolean;
};

function parseLogLines(content: string): OcsfEventDto[] {
  return content
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line) as OcsfEventDto;
      } catch {
        return null;
      }
    })
    .filter((event): event is OcsfEventDto => event !== null);
}

export function TroubleshootingPanel({ backendConnected }: TroubleshootingPanelProps) {
  const [environment, setEnvironment] = useState<EnvironmentStatusDto | null>(null);
  const [diagnostics, setDiagnostics] = useState<ModelRegistryDiagnosticsDto | null>(null);
  const [logFiles, setLogFiles] = useState<LogFileInfoDto[]>([]);
  const [selectedLog, setSelectedLog] = useState("app.log");
  const [logContent, setLogContent] = useState("");
  const [recentEvents, setRecentEvents] = useState<OcsfEventDto[]>([]);
  const [query, setQuery] = useState("");
  const [severity, setSeverity] = useState("all");
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    if (!backendConnected) return;
    setLoading(true);
    setError(null);
    try {
      const [env, registry, files, events] = await Promise.all([
        getEnvironment(),
        getModelsRegistryDiagnostics(),
        listLogFiles(),
        getRecentLogEvents(300),
      ]);
      setEnvironment(env);
      setDiagnostics(registry);
      setLogFiles(files);
      setRecentEvents(events);
      if (selectedLog) {
        const tail = await tailLogFile(selectedLog, 96 * 1024);
        setLogContent(tail.content);
      }
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, [backendConnected, selectedLog]);

  useEffect(() => {
    if (!autoRefresh || !backendConnected) return;
    const timer = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(timer);
  }, [autoRefresh, backendConnected, selectedLog]);

  const mergedEvents = useMemo(() => {
    const fromTail = parseLogLines(logContent);
    const map = new Map<string, OcsfEventDto>();
    for (const event of [...recentEvents, ...fromTail]) {
      const key = `${event.timestamp}:${event.activityName}:${event.message}`;
      map.set(key, event);
    }
    return Array.from(map.values()).sort((a, b) => a.timestamp.localeCompare(b.timestamp));
  }, [logContent, recentEvents]);

  const filteredEvents = useMemo(() => {
    const q = query.trim().toLowerCase();
    return mergedEvents.filter((event) => {
      if (severity !== "all" && event.severity !== severity) return false;
      if (!q) return true;
      return (
        event.message.toLowerCase().includes(q) ||
        event.category.toLowerCase().includes(q) ||
        event.module.toLowerCase().includes(q) ||
        event.activityName.toLowerCase().includes(q)
      );
    });
  }, [mergedEvents, query, severity]);

  const latestError = [...filteredEvents]
    .reverse()
    .find((event) => event.severity === "high" || event.severity === "critical");
  const latestWarning = [...filteredEvents]
    .reverse()
    .find((event) => event.severity === "medium" || event.severity === "low");

  async function openLogFolder() {
    const path = await getLogsFolderPath();
    await navigator.clipboard.writeText(path);
  }

  return (
    <div className="settings-tab-panel troubleshooting-panel">
      <div className="settings-grid">
        <Card>
          <h3 className="card__title">Environment Status</h3>
          {!backendConnected ? (
            <p className="text-muted text-sm">Connect to the desktop backend to load environment status.</p>
          ) : environment ? (
            <dl className="about-list">
              <div><dt>Root</dt><dd className="mono">{shortenPromptLabPath(environment.root, environment.root)}</dd></div>
              <div><dt>Workspaces</dt><dd className="mono">{shortenPromptLabPath(environment.workspaces, environment.root)}</dd></div>
              <div><dt>Models</dt><dd className="mono">{shortenPromptLabPath(environment.models, environment.root)}</dd></div>
              <div><dt>Runtime</dt><dd className="mono">{shortenPromptLabPath(environment.runtime, environment.root)}</dd></div>
              <div><dt>Logs</dt><dd className="mono">{shortenPromptLabPath(environment.logs, environment.root)}</dd></div>
              <div><dt>Database</dt><dd className="mono">{shortenPromptLabPath(environment.database, environment.root)}</dd></div>
            </dl>
          ) : null}
        </Card>

        <Card>
          <h3 className="card__title">Recent Errors</h3>
          {latestError ? (
            <p className="text-danger text-sm">{latestError.message}</p>
          ) : (
            <p className="text-muted text-sm">No recent high-severity events.</p>
          )}
          <h4 className="card__subtitle">Latest Warning</h4>
          {latestWarning ? (
            <p className="text-warning text-sm">{latestWarning.message}</p>
          ) : (
            <p className="text-muted text-sm">No recent warnings.</p>
          )}
        </Card>

        <Card>
          <h3 className="card__title">Registry Diagnostics</h3>
          {diagnostics ? <RegistryDiagnosticsPanel diagnostics={diagnostics} /> : null}
        </Card>

        <Card className="troubleshooting-logs">
          <div className="troubleshooting-logs__header">
            <h3 className="card__title">Live Log Viewer</h3>
            <div className="troubleshooting-logs__actions">
              <label className="settings-toggle">
                <input
                  type="checkbox"
                  checked={autoRefresh}
                  onChange={(e) => setAutoRefresh(e.target.checked)}
                />
                <span>Auto refresh</span>
              </label>
              <Button variant="ghost" onClick={() => void refresh()} disabled={loading}>
                {loading ? "Refreshing…" : "Refresh"}
              </Button>
              <Button variant="secondary" onClick={() => void openLogFolder()}>
                Copy Log Folder Path
              </Button>
            </div>
          </div>

          <div className="troubleshooting-logs__filters">
            <input
              className="input"
              placeholder="Search logs…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <select className="input" value={severity} onChange={(e) => setSeverity(e.target.value)}>
              <option value="all">All severities</option>
              <option value="informational">Informational</option>
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
              <option value="critical">Critical</option>
            </select>
            <select
              className="input mono"
              value={selectedLog}
              onChange={(e) => setSelectedLog(e.target.value)}
            >
              {logFiles.map((file) => (
                <option key={file.path} value={file.name}>
                  {file.name}
                </option>
              ))}
            </select>
          </div>

          {error ? <p className="text-danger text-sm">{error}</p> : null}

          <div className="troubleshooting-logs__table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Time</th>
                  <th>Severity</th>
                  <th>Category</th>
                  <th>Activity</th>
                  <th>Message</th>
                </tr>
              </thead>
              <tbody>
                {filteredEvents.slice(-200).map((event) => (
                  <tr key={`${event.timestamp}-${event.activityName}-${event.message}`}>
                    <td className="mono text-sm">{event.timestamp}</td>
                    <td>{event.severity}</td>
                    <td>{event.category}</td>
                    <td>{event.activityName}</td>
                    <td>{event.message}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      </div>
    </div>
  );
}
