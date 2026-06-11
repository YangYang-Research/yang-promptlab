import type {
  ActivityItem,
  AttackRun,
  DashboardStats,
  DiscoveryJob,
  Finding,
  LocalModel,
  Project,
  Report,
  Target,
} from "@/shared/types";

export type AppSettings = {
  theme: "dark" | "light" | "system";
  pluginsDir: string;
  modelsDir: string;
  offlineMode: boolean;
  autoJudge: boolean;
  telemetry: boolean;
};

export type UiState = {
  sidebarCollapsed: boolean;
  searchQuery: string;
  selectedProjectId: string | null;
  severityFilter: string | null;
};

export type AppDataState = {
  projects: Project[];
  targets: Target[];
  discoveryJobs: DiscoveryJob[];
  attackRuns: AttackRun[];
  findings: Finding[];
  reports: Report[];
  models: LocalModel[];
  activity: ActivityItem[];
  settings: AppSettings;
  ui: UiState;
  backendVersion: string;
  backendConnected: boolean;
};

export type AppAction =
  | { type: "SET_BACKEND"; version: string; connected: boolean }
  | { type: "SET_SEARCH"; query: string }
  | { type: "TOGGLE_SIDEBAR" }
  | { type: "SET_SELECTED_PROJECT"; projectId: string | null }
  | { type: "SET_SEVERITY_FILTER"; severity: string | null }
  | { type: "UPDATE_FINDING_STATUS"; findingId: string; status: Finding["status"] }
  | { type: "UPDATE_SETTING"; key: keyof AppSettings; value: AppSettings[keyof AppSettings] };

export type AppStoreValue = AppDataState & {
  stats: DashboardStats;
  dispatch: (action: AppAction) => void;
};
