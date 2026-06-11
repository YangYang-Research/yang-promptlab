import { Outlet } from "react-router-dom";

import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

export function MainLayout() {
  return (
    <div className="main-layout">
      <Sidebar />
      <div className="main-layout__content">
        <TopBar />
        <main className="main-layout__page">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
