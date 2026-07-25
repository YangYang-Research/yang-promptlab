import { invokeCommand } from "./invoke";

export type ProxySettingsDto = {
  enabled: boolean;
  url: string;
  username?: string | null;
  password?: string | null;
  noProxy: string;
  testUrl: string;
  allowInsecureTls: boolean;
};

export type ProxyTestResultDto = {
  ok: boolean;
  latencyMs: number;
  status?: number | null;
  message: string;
  viaProxy: boolean;
};

export const DEFAULT_PROXY_SETTINGS: ProxySettingsDto = {
  enabled: false,
  url: "",
  username: "",
  password: "",
  noProxy: "localhost,127.0.0.1",
  testUrl: "https://www.google.com/generate_204",
  allowInsecureTls: false,
};

export const getProxySettings = () =>
  invokeCommand<ProxySettingsDto>("proxy_get");

export const setProxySettings = (settings: ProxySettingsDto) =>
  invokeCommand<ProxySettingsDto>("proxy_set", { settings });

export const testProxyConnection = (settings: ProxySettingsDto) =>
  invokeCommand<ProxyTestResultDto>("proxy_test_connection", { settings });
