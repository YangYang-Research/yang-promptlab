import {
  createContext,
  useContext,
  useMemo,
  useReducer,
  type ReactNode,
} from "react";

import {
  computeDashboardStats,
  mockActivity,
  mockAttackRuns,
  mockDiscoveryJobs,
  mockFindings,
  mockModels,
  mockProjects,
  mockReports,
  mockTargets,
} from "@/shared/mock/data";

import type { AppAction, AppDataState, AppStoreValue } from "./types";

const initialState: AppDataState = {
  projects: mockProjects,
  targets: mockTargets,
  discoveryJobs: mockDiscoveryJobs,
  attackRuns: mockAttackRuns,
  findings: mockFindings,
  reports: mockReports,
  models: mockModels,
  activity: mockActivity,
  settings: {
    theme: "dark",
    pluginsDir: "~/.aisec/plugins",
    modelsDir: "~/.aisec/models",
    offlineMode: true,
    autoJudge: true,
    telemetry: false,
  },
  ui: {
    sidebarCollapsed: false,
    searchQuery: "",
    selectedProjectId: "proj-1",
    severityFilter: null,
  },
  backendVersion: "",
  backendConnected: false,
};

function appReducer(state: AppDataState, action: AppAction): AppDataState {
  switch (action.type) {
    case "SET_BACKEND":
      return {
        ...state,
        backendVersion: action.version,
        backendConnected: action.connected,
      };
    case "SET_SEARCH":
      return { ...state, ui: { ...state.ui, searchQuery: action.query } };
    case "TOGGLE_SIDEBAR":
      return {
        ...state,
        ui: { ...state.ui, sidebarCollapsed: !state.ui.sidebarCollapsed },
      };
    case "SET_SELECTED_PROJECT":
      return {
        ...state,
        ui: { ...state.ui, selectedProjectId: action.projectId },
      };
    case "SET_SEVERITY_FILTER":
      return {
        ...state,
        ui: { ...state.ui, severityFilter: action.severity },
      };
    case "UPDATE_FINDING_STATUS":
      return {
        ...state,
        findings: state.findings.map((f) =>
          f.id === action.findingId ? { ...f, status: action.status } : f,
        ),
      };
    case "UPDATE_SETTING":
      return {
        ...state,
        settings: { ...state.settings, [action.key]: action.value },
      };
    default:
      return state;
  }
}

const AppStoreContext = createContext<AppStoreValue | null>(null);

type AppStoreProviderProps = {
  children: ReactNode;
};

export function AppStoreProvider({ children }: AppStoreProviderProps) {
  const [state, dispatch] = useReducer(appReducer, initialState);

  const value = useMemo<AppStoreValue>(() => {
    const stats = computeDashboardStats(
      state.projects,
      state.targets,
      state.findings,
      state.discoveryJobs,
      state.models,
    );
    return { ...state, stats, dispatch };
  }, [state]);

  return (
    <AppStoreContext.Provider value={value}>{children}</AppStoreContext.Provider>
  );
}

export function useAppStore(): AppStoreValue {
  const ctx = useContext(AppStoreContext);
  if (!ctx) {
    throw new Error("useAppStore must be used within AppStoreProvider");
  }
  return ctx;
}

export function useFilteredFindings() {
  const { findings, ui, projects } = useAppStore();
  const query = ui.searchQuery.toLowerCase().trim();

  return findings.filter((f) => {
    if (ui.selectedProjectId && f.projectId !== ui.selectedProjectId) {
      return false;
    }
    if (ui.severityFilter && f.severity !== ui.severityFilter) {
      return false;
    }
    if (!query) {
      return true;
    }
    const project = projects.find((p) => p.id === f.projectId);
    return (
      f.title.toLowerCase().includes(query) ||
      f.targetName.toLowerCase().includes(query) ||
      f.category.toLowerCase().includes(query) ||
      project?.name.toLowerCase().includes(query)
    );
  });
}
