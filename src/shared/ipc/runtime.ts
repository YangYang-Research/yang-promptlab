import { invokeCommand } from "./invoke";

export type DiscoveredModelDto = {
  name: string;
  sizeBytes: number;
  modifiedAt: string | null;
  digest: string | null;
};

export type RuntimeStatusDto = {
  state: string;
  binaryPath: string;
  binaryAvailable: boolean;
  baseUrl: string;
  healthy: boolean;
  installedModels: DiscoveredModelDto[];
  message: string;
};

export function getRuntimeStatus(): Promise<RuntimeStatusDto> {
  return invokeCommand<RuntimeStatusDto>("runtime_status");
}
