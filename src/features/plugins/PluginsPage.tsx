import { useCallback, useEffect, useState } from "react";

import {
  Button,
  Card,
  DataTable,
  PageHeader,
  RefreshButton,
  StatusBadge,
} from "@/shared/components";
import { useAppStore } from "@/app/store/AppStore";
import { toAppError } from "@/shared/errors";
import {
  disablePlugin,
  enablePlugin,
  getPluginsInfo,
  listPlugins,
  refreshPlugins,
  type PluginRecordDto,
  type PluginsInfoDto,
} from "@/shared/ipc/plugins";

export function PluginsPage() {
  const { backendConnected } = useAppStore();
  const [plugins, setPlugins] = useState<PluginRecordDto[]>([]);
  const [info, setInfo] = useState<PluginsInfoDto | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!backendConnected) {
      setPlugins([]);
      setInfo(null);
      return;
    }
    const [listed, summary] = await Promise.all([listPlugins(), getPluginsInfo()]);
    setPlugins(listed);
    setInfo(summary);
  }, [backendConnected]);

  useEffect(() => {
    void load().catch(() => {
      setPlugins([]);
      setInfo(null);
    });
  }, [load]);

  async function handleRefresh() {
    setBusy(true);
    setError(null);
    try {
      const listed = await refreshPlugins();
      setPlugins(listed);
      setInfo(await getPluginsInfo());
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  async function handleToggle(plugin: PluginRecordDto) {
    setBusy(true);
    setError(null);
    try {
      const updated = plugin.enabled
        ? await disablePlugin(plugin.id)
        : await enablePlugin(plugin.id);
      setPlugins((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
      setInfo(await getPluginsInfo());
    } catch (err) {
      setError(toAppError(err).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page plugins-page">
      <PageHeader
        title="Plugins"
        description="Manage attack and judge extensions"
        actions={
          <RefreshButton
            loading={busy}
            error={error}
            disabled={!backendConnected}
            onClick={() => void handleRefresh()}
          />
        }
      />

      {!backendConnected ? (
        <Card>
          <p className="text-muted">Connect to the Tauri backend to manage plugins.</p>
        </Card>
      ) : (
        <>
          {info ? (
            <Card>
              <dl className="about-list">
                <div>
                  <dt>Plugins directory</dt>
                  <dd className="mono">{info.pluginsDir}</dd>
                </div>
                <div>
                  <dt>Installed</dt>
                  <dd>{info.installedCount}</dd>
                </div>
                <div>
                  <dt>Enabled</dt>
                  <dd>{info.enabledCount}</dd>
                </div>
              </dl>
            </Card>
          ) : null}

          {error ? <p className="text-danger text-sm">{error}</p> : null}

          <Card>
            <DataTable
              columns={[
                {
                  key: "name",
                  header: "Plugin",
                  render: (plugin) => (
                    <div>
                      <div>{plugin.name}</div>
                      <div className="text-muted text-sm mono">{plugin.id}</div>
                    </div>
                  ),
                },
                {
                  key: "type",
                  header: "Type",
                  render: (plugin) => plugin.pluginType,
                },
                {
                  key: "version",
                  header: "Version",
                  render: (plugin) => plugin.version,
                },
                {
                  key: "status",
                  header: "Status",
                  render: (plugin) => (
                    <StatusBadge status={plugin.enabled ? "enabled" : "disabled"} />
                  ),
                },
                {
                  key: "actions",
                  header: "",
                  render: (plugin) => (
                    <Button
                      variant="ghost"
                      disabled={busy}
                      onClick={() => void handleToggle(plugin)}
                    >
                      {plugin.enabled ? "Disable" : "Enable"}
                    </Button>
                  ),
                },
              ]}
              rows={plugins}
              keyField="id"
              emptyMessage="No plugins installed. Use Refresh to scan the plugins directory."
            />
          </Card>
        </>
      )}
    </div>
  );
}
