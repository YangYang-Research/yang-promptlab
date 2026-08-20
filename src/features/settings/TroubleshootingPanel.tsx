import { useEffect, useMemo, useState } from "react";

import { Button, Card, RefreshButton, SearchInput, Select } from "@/shared/components";
import { toAppError } from "@/shared/errors";
import { shortenPromptLabPath } from "@/shared/utils/format";
import {
  getDbHealth,
  getEnvironment,
  getRecentLogEvents,
  listLogFiles,
  openLogsFolder,
  tailLogFile,
  type DbHealthDto,
  type EnvironmentStatusDto,
  type LogFileInfoDto,
  type OcsfEventDto,
} from "@/shared/ipc/environment";

import { DatabaseDiagnosticsPanel } from "./DatabaseDiagnosticsPanel";

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
  const [dbHealth, setDbHealth] = useState<DbHealthDto | null>(null);
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
      const [env, health, files, events] = await Promise.all([
        getEnvironment(),
        getDbHealth(),
        listLogFiles(),
        getRecentLogEvents(300),
      ]);
      setEnvironment(env);
      setDbHealth(health);
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
    try {
      await openLogsFolder();
    } catch (err) {
      setError(toAppError(err).message);
    }
  }

  return (
    <div className="settings-tab-panel troubleshooting-panel">
      <div className="settings-sections">
        <Card className="settings-section detail-section">
          <header className="settings-section__header">
            <h2 className="detail-section__title settings-section__title">Health summary</h2>
            <p className="settings-section__lead">
              Recent high-severity events and local database connectivity.
            </p>
          </header>
          <div className="settings-section__body">
            <div className="settings-grid settings-grid--diagnostics">
              <div className="settings-section__panel">
                <h3 className="card__title">Recent errors</h3>
                {latestError ? (
                  <p className="text-danger text-sm">{latestError.message}</p>
                ) : (
                  <p className="text-muted text-sm">No recent high-severity events.</p>
                )}
                <h4 className="card__subtitle">Latest warning</h4>
                {latestWarning ? (
                  <p className="text-warning text-sm">{latestWarning.message}</p>
                ) : (
                  <p className="text-muted text-sm">No recent warnings.</p>
                )}
              </div>

              <div className="settings-section__panel">
                <h3 className="card__title">Database diagnostics</h3>
                {dbHealth ? (
                  <DatabaseDiagnosticsPanel health={dbHealth} root={environment?.root} />
                ) : null}
              </div>
            </div>
          </div>
        </Card>

        <Card className="settings-section detail-section troubleshooting-logs">
          <header className="settings-section__header">
            <h2 className="detail-section__title settings-section__title">Live logs</h2>
            <p className="settings-section__lead">
              Tail application logs with filters. Workspace paths are under Data &amp; storage.
              {environment ? (
                <>
                  {" "}
                  Logs directory:{" "}
                  <span className="mono text-sm">
                    {shortenPromptLabPath(environment.logs, environment.root)}
                  </span>
                </>
              ) : null}
            </p>
          </header>
          <div className="troubleshooting-logs__header">
            <label className="settings-toggle troubleshooting-logs__toggle">
              <input
                type="checkbox"
                checked={autoRefresh}
                onChange={(e) => setAutoRefresh(e.target.checked)}
              />
              <span>Auto refresh</span>
            </label>
            <div className="troubleshooting-logs__actions">
              <RefreshButton
                loading={loading}
                error={error}
                showSuccessToast={false}
                onClick={() => void refresh()}
              />
              <Button variant="secondary" onClick={() => void openLogFolder()}>
                Open log folder
              </Button>
            </div>
          </div>

          <div className="troubleshooting-logs__filters">
            <SearchInput
              value={query}
              onChange={setQuery}
              placeholder="Search logs…"
            />
            <label className="field troubleshooting-logs__field">
              <span className="field__label">Severity</span>
              <Select value={severity} onChange={(e) => setSeverity(e.target.value)}>
                <option value="all">All severities</option>
                <option value="informational">Informational</option>
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
                <option value="critical">Critical</option>
              </Select>
            </label>
            <label className="field troubleshooting-logs__field">
              <span className="field__label">Log file</span>
              <Select
                className="mono"
                value={selectedLog}
                onChange={(e) => setSelectedLog(e.target.value)}
              >
                {logFiles.map((file) => (
                  <option key={file.path} value={file.name}>
                    {file.name}
                  </option>
                ))}
              </Select>
            </label>
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
