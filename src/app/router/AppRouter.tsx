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
const DiscoveryPage = lazy(() =>
  import("@/features/discovery/DiscoveryPage").then((m) => ({ default: m.DiscoveryPage })),
);
const AttacksPage = lazy(() =>
  import("@/features/attacks/AttacksPage").then((m) => ({ default: m.AttacksPage })),
);
const FindingsPage = lazy(() =>
  import("@/features/findings/FindingsPage").then((m) => ({ default: m.FindingsPage })),
);
const ReportsPage = lazy(() =>
  import("@/features/reports/ReportsPage").then((m) => ({ default: m.ReportsPage })),
);
const ModelsPage = lazy(() =>
  import("@/features/models/ModelsPage").then((m) => ({ default: m.ModelsPage })),
);
const PluginsPage = lazy(() =>
  import("@/features/plugins/PluginsPage").then((m) => ({ default: m.PluginsPage })),
);
const JudgeProviderPage = lazy(() =>
  import("@/features/judge/JudgeProviderPage").then((m) => ({ default: m.JudgeProviderPage })),
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
const DiscoveryDetailsPage = lazy(() =>
  import("@/features/discovery/DiscoveryDetailsPage").then((m) => ({
    default: m.DiscoveryDetailsPage,
  })),
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
            path="discovery/:scanId"
            element={
              <Suspense fallback={<PageLoader />}>
                <DiscoveryDetailsPage />
              </Suspense>
            }
          />
          <Route
            path="discovery"
            element={
              <Suspense fallback={<PageLoader />}>
                <DiscoveryPage />
              </Suspense>
            }
          />
          <Route
            path="attacks"
            element={
              <Suspense fallback={<PageLoader />}>
                <AttacksPage />
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
            path="reports"
            element={
              <Suspense fallback={<PageLoader />}>
                <ReportsPage />
              </Suspense>
            }
          />
          <Route
            path="judge"
            element={
              <Suspense fallback={<PageLoader />}>
                <JudgeProviderPage />
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
            path="plugins"
            element={
              <Suspense fallback={<PageLoader />}>
                <PluginsPage />
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
