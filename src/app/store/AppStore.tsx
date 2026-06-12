import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  type ReactNode,
} from "react";

import {
  createProject as createProjectCmd,
  createScan as createScanCmd,
  createTarget as createTargetCmd,
  deleteProject as deleteProjectCmd,
  generateReport as generateReportCmd,
  listEndpoints,
  listFindings,
  listProjects,
  listReports,
  listScans,
  listTargets,
  runDiscovery as runDiscoveryCmd,
  type EndpointDto,
  type FindingDto,
  type ReportDto,
  type ScanDto,
} from "@/shared/ipc";
import { toAppError } from "@/shared/errors";
import { createLogger } from "@/shared/logging";
import { computeDashboardStats } from "@/shared/stats";

import {
  mapEndpoints,
  mapFindings,
  mapProjects,
  mapReports,
  mapScans,
  mapTargets,
} from "./mappers";
import type {
  AppAction,
  AppActions,
  AppDataState,
  AppStoreValue,
  LoadedData,
} from "./types";

const log = createLogger("AppStore");

const initialState: AppDataState = {
  projects: [],
  targets: [],
  scans: [],
  endpoints: [],
  discoveryJobs: [],
  attackRuns: [],
  findings: [],
  reports: [],
  models: [],
  activity: [],
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
    selectedProjectId: null,
    severityFilter: null,
  },
  backendVersion: "",
  backendConnected: false,
  loading: true,
  error: null,
};

function appReducer(state: AppDataState, action: AppAction): AppDataState {
  switch (action.type) {
    case "SET_BACKEND":
      return {
        ...state,
        backendVersion: action.version,
        backendConnected: action.connected,
      };
    case "SET_LOADING":
      return { ...state, loading: action.loading };
    case "SET_ERROR":
      return { ...state, error: action.error };
    case "SET_DATA":
      return { ...state, ...action.data };
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

async function loadAll(): Promise<LoadedData> {
  const projectDtos = await listProjects();

  const [targetGroups, scanGroups, reportGroups] = await Promise.all([
    Promise.all(projectDtos.map((p) => listTargets(p.id))),
    Promise.all(projectDtos.map((p) => listScans(p.id))),
    Promise.all(projectDtos.map((p) => listReports(p.id))),
  ]);

  const targetDtos = targetGroups.flat();
  const scanDtos: ScanDto[] = scanGroups.flat();
  const reportDtos: ReportDto[] = reportGroups.flat();

  const [findingGroups, endpointGroups] = await Promise.all([
    Promise.all(scanDtos.map((s) => listFindings(s.id))),
    Promise.all(scanDtos.map((s) => listEndpoints(s.id))),
  ]);
  const findingDtos: FindingDto[] = findingGroups.flat();
  const endpointDtos: EndpointDto[] = endpointGroups.flat();

  return {
    projects: mapProjects(projectDtos, targetDtos, findingDtos),
    targets: mapTargets(targetDtos),
    scans: mapScans(scanDtos),
    endpoints: mapEndpoints(endpointDtos),
    findings: mapFindings(findingDtos, targetDtos),
    reports: mapReports(reportDtos, projectDtos),
  };
}

export function AppStoreProvider({ children }: AppStoreProviderProps) {
  const [state, dispatch] = useReducer(appReducer, initialState);
  const inFlight = useRef(false);

  const refresh = useCallback(async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    dispatch({ type: "SET_LOADING", loading: true });
    try {
      const data = await loadAll();
      dispatch({ type: "SET_DATA", data });
      dispatch({ type: "SET_ERROR", error: null });
      log.info("workspace data loaded", {
        projects: data.projects.length,
        targets: data.targets.length,
        findings: data.findings.length,
        reports: data.reports.length,
      });
    } catch (error) {
      const appError = toAppError(error);
      log.error("failed to load workspace data", { error: appError });
      dispatch({ type: "SET_ERROR", error: appError.message });
    } finally {
      dispatch({ type: "SET_LOADING", loading: false });
      inFlight.current = false;
    }
  }, []);

  const runMutation = useCallback(
    async (label: string, op: () => Promise<unknown>) => {
      try {
        await op();
        await refresh();
      } catch (error) {
        const appError = toAppError(error);
        log.error(`mutation failed: ${label}`, { error: appError });
        dispatch({ type: "SET_ERROR", error: appError.message });
        throw appError;
      }
    },
    [refresh],
  );

  const actions = useMemo<AppActions>(
    () => ({
      refresh,
      createProject: (name, description) =>
        runMutation("createProject", () => createProjectCmd(name, description)),
      deleteProject: (id) => runMutation("deleteProject", () => deleteProjectCmd(id)),
      createTarget: (projectId, name, targetType, descriptor) =>
        runMutation("createTarget", () =>
          createTargetCmd(projectId, name, targetType, descriptor),
        ),
      createScan: (projectId, name, targetId, status) =>
        runMutation("createScan", () => createScanCmd(projectId, name, targetId, status)),
      generateReport: (projectId, scanId, format, kind) =>
        runMutation("generateReport", () =>
          generateReportCmd(projectId, scanId, format, kind),
        ),
      runDiscovery: async (targetId) => {
        try {
          const result = await runDiscoveryCmd(targetId);
          await refresh();
          return result;
        } catch (error) {
          const appError = toAppError(error);
          log.error("mutation failed: runDiscovery", { error: appError });
          dispatch({ type: "SET_ERROR", error: appError.message });
          throw appError;
        }
      },
    }),
    [refresh, runMutation],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const value = useMemo<AppStoreValue>(() => {
    const stats = computeDashboardStats(
      state.projects,
      state.targets,
      state.findings,
      state.scans,
      state.models,
    );
    return { ...state, stats, dispatch, actions };
  }, [state, actions]);

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
