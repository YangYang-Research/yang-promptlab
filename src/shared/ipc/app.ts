import { invokeCommand } from "./invoke";

export function clearAllAppData(): Promise<void> {
  return invokeCommand<void>("app_clear_all_data");
}
