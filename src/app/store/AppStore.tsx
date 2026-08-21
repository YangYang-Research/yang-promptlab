import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  type ReactNode,
} from "react";

import { listen } from "@tauri-apps/api/event";

import {
  createProject as createProjectCmd,
  createTarget as createTargetCmd,
  deleteProject as deleteProjectCmd,
  deleteTarget as deleteTargetCmd,
  deleteFinding as deleteFindingCmd,
  updateFindingStatus as updateFindingStatusCmd,
  updateProject as updateProjectCmd,
  generateReport as generateReportCmd,
  listFindingsAll,
  listReportsAll,
  listProjects,
  listScans,
  listTargets,
  getScanStatus,
  listModels,
  type FindingDto,
  type ModelEntryDto,
  type ScanDto,
} from "@/shared/ipc";
import { toAppError } from "@/shared/errors";
import { createLogger } from "@/shared/logging";
import { computeDashboardStats } from "@/shared/stats";
import { LOCAL_ACTIVITY_CHANGED_EVENT } from "@/shared/activity/localActivity";
import {
  deriveActivity,
  deriveAttackRuns,
} from "@/shared/dashboardDerived";

import { AppStoreContext } from "./AppStoreContext";
import {
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
import type { LocalModel, ModelStatus } from "@/shared/types";
import { loadThemePreference } from "@/shared/theme/theme";

function mapLocalModels(entries: ModelEntryDto[]): LocalModel[] {
  return entries.map((entry) => ({
    id: entry.id,
    name: entry.name,
    provider: entry.provider,
    sizeGb: entry.sizeGb,
    status: (entry.status as ModelStatus) || "available",
    downloadProgress: entry.status === "downloading" ? 50 : entry.status === "installed" ? 100 : 0,
    quant: entry.format,
    path: entry.path || null,
    sha256: entry.sha256,
  }));
}

const log = createLogger("AppStore");

const initialState: AppDataState = {
  projects: [],
  targets: [],
  scans: [],
  attackRuns: [],
  findings: [],
  reports: [],
  models: [],
  activity: [],
  settings: {
    theme: loadThemePreference(),
    pluginsDir: "~/.promptlab/plugins",
    modelsDir: "~/.promptlab/models",
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
    case "REFRESH_ACTIVITY":
      return {
        ...state,
        activity: deriveActivity(
          state.findings,
          state.scans,
          state.targets,
          state.projects,
          state.reports,
        ),
      };
    default:
      return state;
  }
}

type AppStoreProviderProps = {
  children: ReactNode;
};

async function loadAll(): Promise<LoadedData> {
  const projectDtos = await listProjects();

  const [targetGroups, scanGroups, reportDtos] = await Promise.all([
    Promise.all(projectDtos.map((p) => listTargets(p.id))),
    Promise.all(projectDtos.map((p) => listScans(p.id))),
    listReportsAll(),
  ]);

  const targetDtos = targetGroups.flat();
  const scanDtos: ScanDto[] = scanGroups.flat();

  const [findingGroups, modelEntries] = await Promise.all([
    listFindingsAll(),
    listModels().catch(() => [] as ModelEntryDto[]),
  ]);
  const findingDtos: FindingDto[] = findingGroups;

  const projects = mapProjects(projectDtos, targetDtos, findingDtos);
  const scans = mapScans(scanDtos);
  const targets = mapTargets(targetDtos, scans);
  const findings = mapFindings(findingDtos, targetDtos);

  const runningIds = scans
    .filter((s) => s.status === "running" || s.status === "paused" || s.status === "pending")
    .map((s) => s.id);
  const liveStatuses = await Promise.all(
    runningIds.map((id) => getScanStatus(id).catch(() => null)),
  );
  const liveStatusMap = new Map(
    liveStatuses
      .filter((status): status is NonNullable<typeof status> => status !== null)
      .map((status) => [status.scan_id, status]),
  );

  const reports = mapReports(reportDtos, projectDtos, scanDtos);

  return {
    projects,
    targets,
    scans,
    findings,
    reports,
    models: mapLocalModels(modelEntries),
    attackRuns: deriveAttackRuns(scans, targets, liveStatusMap),
    activity: deriveActivity(findings, scans, targets, projects, reports),
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
      createProject: async (name, description) => {
        try {
          const dto = await createProjectCmd(name, description);
          await refresh();
          return mapProjects([dto], [], [])[0];
        } catch (error) {
          const appError = toAppError(error);
          log.error("mutation failed: createProject", { error: appError });
          dispatch({ type: "SET_ERROR", error: appError.message });
          throw appError;
        }
      },
      deleteProject: (id) => runMutation("deleteProject", () => deleteProjectCmd(id)),
      updateProject: async (id, name, description) => {
        try {
          const dto = await updateProjectCmd(id, name, description);
          await refresh();
          return mapProjects([dto], [], [])[0];
        } catch (error) {
          const appError = toAppError(error);
          log.error("mutation failed: updateProject", { error: appError });
          dispatch({ type: "SET_ERROR", error: appError.message });
          throw appError;
        }
      },
      createTarget: async (projectId, name, targetType, descriptor) => {
        try {
          const dto = await createTargetCmd(projectId, name, targetType, descriptor);
          await refresh();
          return mapTargets([dto])[0];
        } catch (error) {
          const appError = toAppError(error);
          log.error("mutation failed: createTarget", { error: appError });
          dispatch({ type: "SET_ERROR", error: appError.message });
          throw appError;
        }
      },
      deleteTarget: (id) => runMutation("deleteTarget", () => deleteTargetCmd(id)),
      updateFindingStatus: (id, status) =>
        runMutation("updateFindingStatus", () => updateFindingStatusCmd(id, status)),
      deleteFinding: (id) => runMutation("deleteFinding", () => deleteFindingCmd(id)),
      generateReport: (projectId, scanId, format, kind) =>
        runMutation("generateReport", () =>
          generateReportCmd(projectId, scanId, format, kind),
        ),
    }),
    [refresh, runMutation],
  );

  useEffect(() => {
    void refresh();
    let unlisten: (() => void) | undefined;
    void listen("app-data-changed", () => {
      void refresh();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      void unlisten?.();
    };
  }, [refresh]);

  useEffect(() => {
    function onLocalActivityChanged() {
      dispatch({ type: "REFRESH_ACTIVITY" });
    }
    window.addEventListener(LOCAL_ACTIVITY_CHANGED_EVENT, onLocalActivityChanged);
    return () => {
      window.removeEventListener(LOCAL_ACTIVITY_CHANGED_EVENT, onLocalActivityChanged);
    };
  }, []);

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
