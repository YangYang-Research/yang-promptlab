import { NavLink } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { navItems } from "@/app/router/nav";

import { NavIcon } from "./NavIcon";

function NavSection({
  label,
  items,
  collapsed,
  criticalFindings,
}: {
  label: string;
  items: typeof navItems;
  collapsed: boolean;
  criticalFindings: number;
}) {
  if (items.length === 0) return null;

  return (
    <>
      <div className="sidebar__section-label">{!collapsed && label}</div>
      <ul className="sidebar__list">
        {items.map((item) => (
          <li key={item.id}>
            <NavLink
              to={item.path}
              end={item.path === "/"}
              className={({ isActive }) =>
                `sidebar__link ${isActive ? "sidebar__link--active" : ""}`
              }
              title={collapsed ? item.label : undefined}
            >
              <NavIcon name={item.icon} />
              {!collapsed && <span>{item.label}</span>}
              {item.id === "findings" && criticalFindings > 0 && (
                <span className="sidebar__badge">{criticalFindings}</span>
              )}
            </NavLink>
          </li>
        ))}
      </ul>
    </>
  );
}

export function Sidebar() {
  const { ui, stats, dispatch } = useAppStore();
  const mainItems = navItems.filter((i) => i.section === "main");
  const aiItems = navItems.filter((i) => i.section === "ai");
  const advancedItems = navItems.filter((i) => i.section === "advanced");
  const systemItems = navItems.filter((i) => i.section === "system");
  const collapsed = ui.sidebarCollapsed;

  return (
    <aside className={`sidebar ${collapsed ? "sidebar--collapsed" : ""}`}>
      <div className="sidebar__brand">
        <div className="sidebar__logo" aria-hidden="true">
          <svg viewBox="0 0 32 32" width="28" height="28">
            <rect width="32" height="32" rx="8" fill="#2563eb" />
            <path d="M8 20l4-8 4 5 4-9 4 12" stroke="#fff" strokeWidth="2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </div>
        {!collapsed && (
          <div className="sidebar__brand-text">
            <span className="sidebar__name">AISec</span>
            <span className="sidebar__tagline">AI Security Platform</span>
          </div>
        )}
      </div>

      <nav className="sidebar__nav" aria-label="Main navigation">
        <NavSection
          label="Workspace"
          items={mainItems}
          collapsed={collapsed}
          criticalFindings={stats.criticalFindings}
        />
        <NavSection label="AI Security Engine" items={aiItems} collapsed={collapsed} criticalFindings={0} />
        <NavSection label="Advanced" items={advancedItems} collapsed={collapsed} criticalFindings={0} />
        <NavSection label="System" items={systemItems} collapsed={collapsed} criticalFindings={0} />
      </nav>

      <div className="sidebar__footer">
        <button
          type="button"
          className="sidebar__collapse-btn"
          onClick={() => dispatch({ type: "TOGGLE_SIDEBAR" })}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          <svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true">
            <path
              d={collapsed ? "M7 4l6 6-6 6" : "M13 4L7 10l6 6"}
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
