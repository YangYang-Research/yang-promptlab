import type {
  ActivityItem,
  AttackRun,
  DashboardStats,
  DiscoveredEndpoint,
  DiscoveryJob,
  Finding,
  LocalModel,
  Project,
  Report,
  ScanRun,
  Target,
} from "@/shared/types";
import type { AttackRunDto, DiscoveryRunDto } from "@/shared/ipc";

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
  scans: ScanRun[];
  endpoints: DiscoveredEndpoint[];
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
  loading: boolean;
  error: string | null;
};

export type LoadedData = Pick<
  AppDataState,
  "projects" | "targets" | "scans" | "endpoints" | "findings" | "reports"
>;

export type AppAction =
  | { type: "SET_BACKEND"; version: string; connected: boolean }
  | { type: "SET_LOADING"; loading: boolean }
  | { type: "SET_ERROR"; error: string | null }
  | { type: "SET_DATA"; data: LoadedData }
  | { type: "SET_SEARCH"; query: string }
  | { type: "TOGGLE_SIDEBAR" }
  | { type: "SET_SELECTED_PROJECT"; projectId: string | null }
  | { type: "SET_SEVERITY_FILTER"; severity: string | null }
  | { type: "UPDATE_FINDING_STATUS"; findingId: string; status: Finding["status"] }
  | { type: "UPDATE_SETTING"; key: keyof AppSettings; value: AppSettings[keyof AppSettings] };

export type AppActions = {
  refresh: () => Promise<void>;
  createProject: (name: string, description?: string | null) => Promise<Project>;
  updateProject: (id: string, name?: string | null, description?: string | null) => Promise<Project>;
  deleteProject: (id: string) => Promise<void>;
  createTarget: (
    projectId: string,
    name: string,
    targetType: string,
    descriptor?: unknown,
  ) => Promise<Target>;
  createScan: (
    projectId: string,
    name: string,
    targetId?: string | null,
    status?: string | null,
  ) => Promise<void>;
  generateReport: (
    projectId: string,
    scanId: string,
    format?: string,
    kind?: string,
  ) => Promise<void>;
  runDiscovery: (targetId: string) => Promise<DiscoveryRunDto>;
  runPromptInjection: (endpointId: string) => Promise<AttackRunDto>;
};

export type AppStoreValue = AppDataState & {
  stats: DashboardStats;
  dispatch: (action: AppAction) => void;
  actions: AppActions;
};
