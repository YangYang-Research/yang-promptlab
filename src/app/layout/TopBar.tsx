import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useNavigate } from "react-router-dom";

import { useAppStore } from "@/app/store/AppStore";
import { SearchInput } from "@/shared/components";
import { searchWorkspace } from "@/shared/ipc";

import {
  asGlobalSearchHit,
  groupSearchHits,
  type GlobalSearchHit,
} from "./globalSearch";

const SEARCH_DEBOUNCE_MS = 180;

export function TopBar() {
  const { ui, dispatch, backendConnected, backendVersion, stats } = useAppStore();
  const navigate = useNavigate();
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [hits, setHits] = useState<GlobalSearchHit[]>([]);
  const [searching, setSearching] = useState(false);

  const groups = useMemo(() => groupSearchHits(hits), [hits]);
  const orderedHits = useMemo(() => groups.flatMap((group) => group.hits), [groups]);
  const hitIndexById = useMemo(() => {
    const map = new Map<string, number>();
    orderedHits.forEach((hit, index) => map.set(hit.id, index));
    return map;
  }, [orderedHits]);
  const showResults = open && ui.searchQuery.trim().length > 0;

  useEffect(() => {
    const query = ui.searchQuery.trim();
    if (!query || !backendConnected) {
      setHits([]);
      setSearching(false);
      return;
    }

    let cancelled = false;
    setSearching(true);
    const timer = window.setTimeout(() => {
      void searchWorkspace(query)
        .then((rows) => {
          if (cancelled) return;
          setHits(rows.flatMap((row) => {
            const hit = asGlobalSearchHit(row);
            return hit ? [hit] : [];
          }));
        })
        .catch(() => {
          if (!cancelled) setHits([]);
        })
        .finally(() => {
          if (!cancelled) setSearching(false);
        });
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [ui.searchQuery, backendConnected]);

  useEffect(() => {
    setActiveIndex(0);
  }, [hits]);

  useEffect(() => {
    if (!showResults) return;

    function onPointerDown(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [showResults]);

  function goTo(hit: GlobalSearchHit) {
    setOpen(false);
    dispatch({ type: "SET_SEARCH", query: "" });
    setHits([]);
    navigate(hit.to);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      setOpen(false);
      if (ui.searchQuery) dispatch({ type: "SET_SEARCH", query: "" });
      return;
    }
    if (!orderedHits.length) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setOpen(true);
      setActiveIndex((index) => (index + 1) % orderedHits.length);
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setOpen(true);
      setActiveIndex((index) => (index - 1 + orderedHits.length) % orderedHits.length);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const hit = orderedHits[activeIndex] ?? orderedHits[0];
      if (hit) goTo(hit);
    }
  }

  return (
    <header className="topbar">
      <div className="topbar__left" aria-hidden="true" />

      <div className="topbar__center">
        <div className="topbar-search" ref={rootRef}>
          <SearchInput
            value={ui.searchQuery}
            onChange={(query) => {
              dispatch({ type: "SET_SEARCH", query });
              setOpen(true);
            }}
            placeholder="Search projects, targets, scans, findings, reports, techniques…"
            inputProps={{
              role: "combobox",
              "aria-expanded": showResults,
              "aria-controls": "topbar-search-results",
              "aria-autocomplete": "list",
              "aria-activedescendant":
                showResults && orderedHits[activeIndex] ? orderedHits[activeIndex].id : undefined,
              onFocus: () => setOpen(true),
              onKeyDown,
            }}
          />
          {showResults ? (
            <div
              className="topbar-search__results"
              id="topbar-search-results"
              role="listbox"
              aria-label="Search results"
            >
              {searching && orderedHits.length === 0 ? (
                <p className="topbar-search__empty">Searching…</p>
              ) : orderedHits.length === 0 ? (
                <p className="topbar-search__empty">No matching projects, targets, scans, findings, reports, or techniques.</p>
              ) : (
                groups.map((group) => (
                  <section
                    key={group.kind}
                    className="topbar-search__group"
                    role="group"
                    aria-label={group.label}
                  >
                    <p className="topbar-search__group-label">{group.label}</p>
                    {group.hits.map((hit) => {
                      const index = hitIndexById.get(hit.id) ?? 0;
                      return (
                        <button
                          key={hit.id}
                          id={hit.id}
                          type="button"
                          role="option"
                          aria-selected={index === activeIndex}
                          className={`topbar-search__hit${index === activeIndex ? " topbar-search__hit--active" : ""}`}
                          onMouseEnter={() => setActiveIndex(index)}
                          onMouseDown={(event) => event.preventDefault()}
                          onClick={() => goTo(hit)}
                        >
                          <span className="topbar-search__hit-title">{hit.title}</span>
                          {hit.subtitle ? (
                            <span className="topbar-search__hit-subtitle">{hit.subtitle}</span>
                          ) : null}
                        </button>
                      );
                    })}
                  </section>
                ))
              )}
            </div>
          ) : null}
        </div>
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
