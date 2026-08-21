import { useRef } from "react";
import { Outlet } from "react-router-dom";

import { SetupChecklistPanel } from "@/features/setup/SetupChecklistPanel";

import { BackToTopButton } from "./BackToTopButton";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

export function MainLayout() {
  const pageRef = useRef<HTMLElement>(null);

  return (
    <div className="main-layout">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <Sidebar />
      <div className="main-layout__content">
        <TopBar />
        <main id="main-content" ref={pageRef} className="main-layout__page" tabIndex={-1}>
          <Outlet />
        </main>
      </div>
      <div className="main-layout__fab-stack">
        <SetupChecklistPanel />
        <BackToTopButton scrollRootRef={pageRef} />
      </div>
    </div>
  );
}
