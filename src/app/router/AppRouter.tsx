import { lazy, Suspense } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";

import { MainLayout } from "@/app/layout/MainLayout";

const DashboardPage = lazy(() =>
  import("@/features/dashboard/DashboardPage").then((m) => ({ default: m.DashboardPage })),
);
const ProjectsPage = lazy(() =>
  import("@/features/projects/ProjectsPage").then((m) => ({ default: m.ProjectsPage })),
);
const ProjectDetailsPage = lazy(() =>
  import("@/features/projects/ProjectDetailsPage").then((m) => ({
    default: m.ProjectDetailsPage,
  })),
);
const TargetsPage = lazy(() =>
  import("@/features/targets/TargetsPage").then((m) => ({ default: m.TargetsPage })),
);
const TargetDetailsPage = lazy(() =>
  import("@/features/targets/TargetDetailsPage").then((m) => ({
    default: m.TargetDetailsPage,
  })),
);
const FindingsPage = lazy(() =>
  import("@/features/findings/FindingsPage").then((m) => ({ default: m.FindingsPage })),
);
const FindingDetailsPage = lazy(() =>
  import("@/features/findings/FindingDetailsPage").then((m) => ({
    default: m.FindingDetailsPage,
  })),
);
const ReportsPage = lazy(() =>
  import("@/features/reports/ReportsPage").then((m) => ({ default: m.ReportsPage })),
);
const ReportDetailsPage = lazy(() =>
  import("@/features/reports/ReportDetailsPage").then((m) => ({
    default: m.ReportDetailsPage,
  })),
);
const ModelsPage = lazy(() =>
  import("@/features/models/ModelsPage").then((m) => ({ default: m.ModelsPage })),
);
const AttackCategoriesPage = lazy(() =>
  import("@/features/attack-catalog/AttackCategoriesPage").then((m) => ({
    default: m.AttackCategoriesPage,
  })),
);
const MutatorsPage = lazy(() =>
  import("@/features/mutators/MutatorsPage").then((m) => ({ default: m.MutatorsPage })),
);
const AgentTracePage = lazy(() =>
  import("@/features/agent-trace/AgentTracePage").then((m) => ({
    default: m.AgentTracePage,
  })),
);
const AgentTraceDetailPage = lazy(() =>
  import("@/features/agent-trace/AgentTraceDetailPage").then((m) => ({
    default: m.AgentTraceDetailPage,
  })),
);
const AIRuntimePage = lazy(() =>
  import("@/features/runtime/AIRuntimePage").then((m) => ({ default: m.AIRuntimePage })),
);
const YazgChatPage = lazy(() =>
  import("@/features/yazg/YazgChatPage").then((m) => ({ default: m.YazgChatPage })),
);
const SettingsPage = lazy(() =>
  import("@/features/settings/SettingsPage").then((m) => ({ default: m.SettingsPage })),
);
const ScanWizardPage = lazy(() =>
  import("@/features/scans/ScanWizardPage").then((m) => ({ default: m.ScanWizardPage })),
);
const ScansPage = lazy(() =>
  import("@/features/scans/ScansPage").then((m) => ({ default: m.ScansPage })),
);
const ScanDetailsPage = lazy(() =>
  import("@/features/scans/ScanDetailsPage").then((m) => ({ default: m.ScanDetailsPage })),
);

function PageLoader() {
  return (
    <div className="page-loader">
      <div className="page-loader__spinner" />
      <span>Loading…</span>
    </div>
  );
}

export function AppRouter() {
  return (
    <HashRouter>
      <Routes>
        <Route element={<MainLayout />}>
          <Route
            index
            element={
              <Suspense fallback={<PageLoader />}>
                <DashboardPage />
              </Suspense>
            }
          />
          <Route
            path="projects"
            element={
              <Suspense fallback={<PageLoader />}>
                <ProjectsPage />
              </Suspense>
            }
          />
          <Route
            path="projects/:projectId"
            element={
              <Suspense fallback={<PageLoader />}>
                <ProjectDetailsPage />
              </Suspense>
            }
          />
          <Route
            path="scans"
            element={
              <Suspense fallback={<PageLoader />}>
                <ScansPage />
              </Suspense>
            }
          />
          <Route
            path="scans/new"
            element={
              <Suspense fallback={<PageLoader />}>
                <ScanWizardPage />
              </Suspense>
            }
          />
          <Route
            path="scans/:scanId"
            element={
              <Suspense fallback={<PageLoader />}>
                <ScanDetailsPage />
              </Suspense>
            }
          />
          <Route
            path="targets/:targetId"
            element={
              <Suspense fallback={<PageLoader />}>
                <TargetDetailsPage />
              </Suspense>
            }
          />
          <Route
            path="targets"
            element={
              <Suspense fallback={<PageLoader />}>
                <TargetsPage />
              </Suspense>
            }
          />
          <Route
            path="findings"
            element={
              <Suspense fallback={<PageLoader />}>
                <FindingsPage />
              </Suspense>
            }
          />
          <Route
            path="findings/:findingId"
            element={
              <Suspense fallback={<PageLoader />}>
                <FindingDetailsPage />
              </Suspense>
            }
          />
          <Route
            path="reports"
            element={
              <Suspense fallback={<PageLoader />}>
                <ReportsPage />
              </Suspense>
            }
          />
          <Route
            path="reports/:reportId"
            element={
              <Suspense fallback={<PageLoader />}>
                <ReportDetailsPage />
              </Suspense>
            }
          />
          <Route
            path="runtime"
            element={
              <Suspense fallback={<PageLoader />}>
                <AIRuntimePage />
              </Suspense>
            }
          />
          <Route
            path="yazg"
            element={
              <Suspense fallback={<PageLoader />}>
                <YazgChatPage />
              </Suspense>
            }
          />
          <Route
            path="models"
            element={
              <Suspense fallback={<PageLoader />}>
                <ModelsPage />
              </Suspense>
            }
          />
          <Route
            path="attack-categories"
            element={
              <Suspense fallback={<PageLoader />}>
                <AttackCategoriesPage />
              </Suspense>
            }
          />
          <Route
            path="mutators"
            element={
              <Suspense fallback={<PageLoader />}>
                <MutatorsPage />
              </Suspense>
            }
          />
          <Route
            path="agent-trace"
            element={
              <Suspense fallback={<PageLoader />}>
                <AgentTracePage />
              </Suspense>
            }
          />
          <Route
            path="agent-trace/:traceId"
            element={
              <Suspense fallback={<PageLoader />}>
                <AgentTraceDetailPage />
              </Suspense>
            }
          />
          <Route
            path="settings"
            element={
              <Suspense fallback={<PageLoader />}>
                <SettingsPage />
              </Suspense>
            }
          />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
