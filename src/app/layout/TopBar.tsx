import { useAppStore } from "@/app/store/AppStore";
import { SearchInput } from "@/shared/components";

export function TopBar() {
  const { ui, dispatch, backendConnected, backendVersion, stats } = useAppStore();

  return (
    <header className="topbar">
      <div className="topbar__left" aria-hidden="true" />

      <div className="topbar__center">
        <SearchInput
          value={ui.searchQuery}
          onChange={(query) => dispatch({ type: "SET_SEARCH", query })}
          placeholder="Search findings, targets, projects…"
        />
      </div>

      <div className="topbar__right">
        {stats.runningScans > 0 && (
          <span className="topbar__status topbar__status--running">
            {stats.runningScans} scan{stats.runningScans > 1 ? "s" : ""} running
          </span>
        )}
        <span
          className={`topbar__connection ${backendConnected ? "topbar__connection--online" : ""}`}
          title={backendConnected ? `Backend v${backendVersion}` : "Backend offline — mock data"}
        >
          <span className="topbar__connection-dot" />
          {backendConnected ? "Connected" : "Mock mode"}
        </span>
      </div>
    </header>
  );
}
