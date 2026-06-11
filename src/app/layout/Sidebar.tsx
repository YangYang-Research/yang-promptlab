import { NavLink } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { navItems } from "@/app/router/nav";

import { NavIcon } from "./NavIcon";

export function Sidebar() {
  const { ui, stats, dispatch } = useAppStore();
  const mainItems = navItems.filter((i) => i.section === "main");
  const systemItems = navItems.filter((i) => i.section === "system");

  return (
    <aside className={`sidebar ${ui.sidebarCollapsed ? "sidebar--collapsed" : ""}`}>
      <div className="sidebar__brand">
        <div className="sidebar__logo" aria-hidden="true">
          <svg viewBox="0 0 32 32" width="28" height="28">
            <rect width="32" height="32" rx="8" fill="#2563eb" />
            <path d="M8 20l4-8 4 5 4-9 4 12" stroke="#fff" strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </div>
        {!ui.sidebarCollapsed && (
          <div className="sidebar__brand-text">
            <span className="sidebar__name">AISec</span>
            <span className="sidebar__tagline">AI Security Platform</span>
          </div>
        )}
      </div>

      <nav className="sidebar__nav" aria-label="Main navigation">
        <ul className="sidebar__list">
          {mainItems.map((item) => (
            <li key={item.id}>
              <NavLink
                to={item.path}
                end={item.path === "/"}
                className={({ isActive }) =>
                  `sidebar__link ${isActive ? "sidebar__link--active" : ""}`
                }
                title={ui.sidebarCollapsed ? item.label : undefined}
              >
                <NavIcon name={item.icon} />
                {!ui.sidebarCollapsed && <span>{item.label}</span>}
                {item.id === "findings" && stats.criticalFindings > 0 && (
                  <span className="sidebar__badge">{stats.criticalFindings}</span>
                )}
              </NavLink>
            </li>
          ))}
        </ul>

        <div className="sidebar__section-label">
          {!ui.sidebarCollapsed && "System"}
        </div>
        <ul className="sidebar__list">
          {systemItems.map((item) => (
            <li key={item.id}>
              <NavLink
                to={item.path}
                className={({ isActive }) =>
                  `sidebar__link ${isActive ? "sidebar__link--active" : ""}`
                }
                title={ui.sidebarCollapsed ? item.label : undefined}
              >
                <NavIcon name={item.icon} />
                {!ui.sidebarCollapsed && <span>{item.label}</span>}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>

      <div className="sidebar__footer">
        <button
          type="button"
          className="sidebar__collapse-btn"
          onClick={() => dispatch({ type: "TOGGLE_SIDEBAR" })}
          aria-label={ui.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          <svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true">
            <path
              d={ui.sidebarCollapsed ? "M7 4l6 6-6 6" : "M13 4L7 10l6 6"}
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            />
          </svg>
        </button>
      </div>
    </aside>
  );
}
