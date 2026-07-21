import { Outlet } from "react-router-dom";

import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

export function MainLayout() {
  return (
    <div className="main-layout">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <Sidebar />
      <div className="main-layout__content">
        <TopBar />
        <main id="main-content" className="main-layout__page" tabIndex={-1}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
