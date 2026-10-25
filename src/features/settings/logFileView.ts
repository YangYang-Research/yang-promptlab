/** Category → file. Mirrors `category_file` in promptlab-core `event_log.rs`. */
const CATEGORY_LOG_FILE: Record<string, string> = {
  application: "app.log",
  system: "system.log",
  runtime: "runtime.log",
  models: "models.log",
  authentication: "auth.log",
  harness: "harness.log",
  planner: "planner.log",
  payload_generator: "payload.log",
  attack_engine: "attack.log",
  judge: "judge.log",
  workspace: "workspace.log",
  projects: "projects.log",
  plugins: "plugins.log",
  settings: "settings.log",
  user_interface: "ui.log",
  scan: "scan.log",
  agent: "agents.log",
};

type AgentLogEvent = {
  category?: string;
  component?: string;
  agentName?: string | null;
  attributes?: Record<string, unknown>;
};

/** Name of the agent that wrote the line. Empty when the event is not an agent log. */
export function agentLogName(event: AgentLogEvent): string {
  const named = event.agentName?.trim();
  if (named) return named;
  const display = event.attributes?.agentDisplay;
  if (typeof display === "string" && display.trim()) return display.trim();
  if (event.category === "agent") return event.component?.trim() ?? "";
  return "";
}

/** app.log receives every category. Other files only receive their own category. */
export function eventBelongsToLogFile(category: string, fileName: string): boolean {
  if (fileName === "app.log") return true;
  return (CATEGORY_LOG_FILE[category] ?? "app.log") === fileName;
}
