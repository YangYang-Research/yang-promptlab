import { invokeCommand } from "./invoke";

export type PluginRecordDto = {
  id: string;
  name: string;
  version: string;
  apiVersion: string;
  pluginType: string;
  language: string;
  state: string;
  enabled: boolean;
  installPath: string;
  hooks: string[];
};

export type PluginsInfoDto = {
  pluginsDir: string;
  installedCount: number;
  enabledCount: number;
  discoveryCount: number;
  attackCount: number;
  judgeCount: number;
};

export function listPlugins(): Promise<PluginRecordDto[]> {
  return invokeCommand<PluginRecordDto[]>("plugins_list");
}

export function refreshPlugins(): Promise<PluginRecordDto[]> {
  return invokeCommand<PluginRecordDto[]>("plugins_refresh");
}

export function enablePlugin(pluginId: string): Promise<PluginRecordDto> {
  return invokeCommand<PluginRecordDto>("plugins_enable", { pluginId });
}

export function disablePlugin(pluginId: string): Promise<PluginRecordDto> {
  return invokeCommand<PluginRecordDto>("plugins_disable", { pluginId });
}

export function getPluginsInfo(): Promise<PluginsInfoDto> {
  return invokeCommand<PluginsInfoDto>("plugins_info");
}
