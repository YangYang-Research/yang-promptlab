# AISec UX Consistency Report

**Date:** 2026-06-13  
**Scope:** Frontend UI (`src/`) after Iteration 5 UX refinements  
**Method:** Static audit of page components, shared design-system primitives, and global styles  
**Note:** This document describes findings only. No code was modified as part of this audit.

---

## Executive Summary

Iteration 5 introduced a coherent design direction: icon-based back navigation on detail pages, shared `ViewModeToggle`, `Pagination`, `ListCard`, and `ActionsDropdown`. Adoption is **partial**. List/table pages (Projects, Scans, Targets, Discovery) largely follow the new pattern; several modules (Attacks, Reports, Findings, Dashboard, Models, Scan Wizard) remain on older or bespoke layouts.

The highest-impact gaps are:

1. **Incomplete back-navigation coverage** on detail-page loading and error routes  
2. **Refresh button variant split** (`ghost` vs `secondary`) across list pages  
3. **List view fragmentation** — three different card/list implementations instead of one  
4. **Empty-state component under-styling** (missing CSS for description/action slots)  
5. **Pagination applied unevenly** (Discovery list mode, Attacks, Models, Dashboard have none)

---

## Pattern Coverage Matrix

| Page | Back icon | ContentToolbar | Pagination | ListCard | EmptyState | Refresh variant |
|------|-----------|----------------|------------|----------|------------|-----------------|
| Projects | — | Yes | Yes | Yes | Partial | `ghost` |
| Project Details | Yes | — | — | — | Yes | — |
| Scans | — | Yes | Yes | No (monitor cards) | Yes | `secondary` |
| Scan Details | Yes (loaded) | — | — | — | Yes | — |
| Targets | — | Yes | Yes | Yes | No (plain Card) | `ghost` |
| Target Details | Yes | — | — | — | Yes | — |
| Discovery | — | Yes | Table only | No (tree) | Yes | `secondary` |
| Discovery Details | Yes (loaded) | — | — | — | Partial | `ghost` |
| Findings | — | No | Yes | No | Yes | `secondary` |
| Reports | — | No | Yes (×2) | No | Partial | `secondary` |
| Attacks | — | No | No | No (discovery-card) | Yes | — |
| Dashboard | — | No | No | No | No | — |
| Models | — | No | No | No (model-card) | No | — |
| Settings | — | No | No | No | No | — |
| Scan Wizard | No | No | No | No | No | — |

---

## 1. Navigation

### 1.1 Detail pages missing back icon during loading

| | |
|---|---|
| **Issue** | `ScanDetailsPage` and `DiscoveryDetailsPage` render a loading header without `backTo`. Users cannot navigate away while data loads. `ProjectDetailsPage` and `TargetDetailsPage` correctly include `backTo` on loading states. |
| **Severity** | **High** |
| **Proposed fix** | Pass `backTo` on all detail-page loading and error headers (`/scans`, `/discovery`, `/targets`, `/projects`). |

### 1.2 Detail pages missing back icon on fatal empty routes

| | |
|---|---|
| **Issue** | When `scanId` or discovery ID is missing, pages show `EmptyState` without a back control. Project/Target not-found states include `PageHeader` + back. |
| **Severity** | **Medium** |
| **Proposed fix** | Wrap all not-found detail views in `PageHeader` with `backTo` pointing to the parent list route. |

### 1.3 Scan Wizard has no header-level exit

| | |
|---|---|
| **Issue** | `ScanWizardPage` uses footer text button “Back” (step navigation) but no icon back to `/scans`. Users must complete or abandon via sidebar. |
| **Severity** | **Medium** |
| **Proposed fix** | Add `PageHeader` `backTo="/scans"` (with unsaved-work confirmation if session state exists). |

### 1.4 Duplicate title chrome (TopBar + PageHeader)

| | |
|---|---|
| **Issue** | Detail pages show entity name in `PageHeader` and generic route label (“Project Details”, “Scan Details”) in `TopBar`. Creates redundant hierarchy. |
| **Severity** | **Low** |
| **Proposed fix** | On detail routes, set TopBar title to entity name (from store/route param) or hide TopBar title when page header includes back + title. |

### 1.5 Sidebar icon collision

| | |
|---|---|
| **Issue** | `nav.ts` assigns the same icon (`discovery`) to both **Scans** and **Discovery** nav items. |
| **Severity** | **Low** |
| **Proposed fix** | Assign distinct icons (e.g. `scans` vs `discovery`) in `NavIcon` mapping. |

---

## 2. Headers

### 2.1 Refresh button variant inconsistency

| | |
|---|---|
| **Issue** | Projects and Targets use `Button variant="ghost"` for Refresh; Scans, Discovery, Findings, and Reports use `variant="secondary"`. Discovery Details uses `ghost` without `disabled={loading}`. |
| **Severity** | **High** |
| **Proposed fix** | Standardize list-page Refresh as `secondary`, always `disabled={loading}`, optional “Refreshing…” label on Projects/Targets pattern applied everywhere. |

### 2.2 Refresh label inconsistency

| | |
|---|---|
| **Issue** | Projects and Targets show “Refreshing…” while loading; other pages keep static “Refresh” text. |
| **Severity** | **Medium** |
| **Proposed fix** | Adopt one pattern app-wide: either dynamic label or spinner icon inside button. |

### 2.3 Header actions wrapper inconsistency

| | |
|---|---|
| **Issue** | Some pages wrap actions in `<div className="page-actions">` (Scans, Project Details); others use bare fragments (Projects, Targets, Findings). Attacks embeds full workflow controls (`discovery-controls`) in the header. |
| **Severity** | **Medium** |
| **Proposed fix** | Always use `page-actions` for right-aligned header clusters; move Attacks launch controls below header (similar to post–Iteration 4 Discovery pattern). |

### 2.4 Detail header title semantics

| | |
|---|---|
| **Issue** | Loaded detail pages use entity names (project name, target name, scan name). Loading/error states use generic labels (“Scan Details”, “Project Details”). Discovery Details always shows “Discovery Details” even when loaded. |
| **Severity** | **Medium** |
| **Proposed fix** | Use entity-derived titles when available (e.g. discovery run label or target URL); reserve generic labels for loading skeleton only. |

### 2.5 Actions dropdown lacks visible label

| | |
|---|---|
| **Issue** | `ProjectDetailsPage` exposes actions via icon-only `ActionsDropdown` (⋮). No text affordance; `ariaLabel` is “Actions” but visual pattern differs from “New Scan” text button beside it. |
| **Severity** | **Medium** |
| **Proposed fix** | Use labeled trigger: `Button variant="secondary"` + chevron, or icon + “Actions” text per design spec. |

---

## 3. Buttons

### 3.1 Primary action naming

| | |
|---|---|
| **Issue** | Primary CTAs vary: “New Project”, “New Scan”, “Add Target”, “Launch Attack”, “Download Model”, “Start Scan” (wizard). No shared verb convention. |
| **Severity** | **Low** |
| **Proposed fix** | Document CTA vocabulary: **New** for create flows, **Add** for sub-resources, **Run/Launch** for execution. Align wizard footer with header (“Start Scan” vs “New Scan”). |

### 3.2 Destructive actions placement

| | |
|---|---|
| **Issue** | Projects list exposes inline **Delete** in table/list rows. Project Details places **Delete** inside overflow menu. Same action, different discovery patterns. |
| **Severity** | **Medium** |
| **Proposed fix** | List pages: icon delete or overflow menu only (remove redundant “View Details” if row click already navigates). Details: keep destructive actions in dropdown with confirmation modal. |

### 3.3 Redundant “View Details” with row click

| | |
|---|---|
| **Issue** | Projects table supports row click navigation **and** explicit “View Details” button. Doubles affordance without adding capability. |
| **Severity** | **Low** |
| **Proposed fix** | Remove “View Details” from table; keep row click. Retain explicit button only in list cards where entire card is clickable but actions need stopPropagation. |

### 3.4 Table action buttons vs icon buttons

| | |
|---|---|
| **Issue** | Discovery table uses `IconButton` + `IconDiscovery` for Run Discovery. Projects table uses text `Button` for Delete/View Details. Iteration 5 spec called for consistent table action icon style. |
| **Severity** | **Medium** |
| **Proposed fix** | Introduce shared `TableIconButton` set (view, delete, run) and migrate row actions. |

### 3.5 Non-functional header buttons

| | |
|---|---|
| **Issue** | Models page (“Browse HuggingFace”, “Download Model”) and Settings (“View Logs”) render buttons with no handlers. |
| **Severity** | **Medium** |
| **Proposed fix** | Wire handlers or disable with “Coming soon” tooltip until implemented. |

---

## 4. Dropdowns

### 4.1 Single ActionsDropdown adoption

| | |
|---|---|
| **Issue** | Only `ProjectDetailsPage` uses `ActionsDropdown`. No shared filter dropdown, no row-level overflow menus elsewhere. |
| **Severity** | **Medium** |
| **Proposed fix** | Extend `ActionsDropdown` (or alias `OverflowMenu`) to Scans/Targets row actions and detail pages that will gain future actions. |

### 4.1 Attacks page header still uses inline selects + primary CTA

| | |
|---|---|
| **Issue** | Attacks mirrors pre–Iteration 4 Discovery pattern (filters + launch in header). Discovery was refactored to contextual row actions; Attacks was not. |
| **Severity** | **High** |
| **Proposed fix** | Move endpoint/category selectors into page body toolbar; launch from table/list row actions. |

### 4.2 Findings filter controls vs shared Select

| | |
|---|---|
| **Issue** | Findings uses shared `Select` for Project/Scan filters but custom `filter-chip` buttons for severity/status. Visually distinct from other filter UIs. |
| **Severity** | **Low** |
| **Proposed fix** | Either document chips as the filter pattern for enum fields, or replace with segmented control component shared across Findings/Attacks. |

---

## 5. Tables

### 5.1 DataTable empty message vs EmptyState

| | |
|---|---|
| **Issue** | Some pages use `DataTable` `emptyMessage` strings; others wrap `EmptyState` in `Card` (Reports export section). Targets empty page bypasses both and uses plain `<p>` in Card. |
| **Severity** | **High** |
| **Proposed fix** | When zero rows and not loading, render full-page or in-card `EmptyState` with optional CTA. Reserve `emptyMessage` for filtered-empty within paginated datasets. |

### 5.2 Reports dual-table layout without section pagination unity

| | |
|---|---|
| **Issue** | Reports page has two independent tables each with its own pagination and page-size preference keys. No shared section header pattern with `ContentToolbar`. |
| **Severity** | **Medium** |
| **Proposed fix** | Use consistent subsection header (`reports-section__header`) + optional collapsed pagination footer; consider unified page-size default display. |

### 5.3 Detail page embedded tables unpaginated

| | |
|---|---|
| **Issue** | Project Details “Recent Targets”, Scan/Discovery Details endpoint tables show fixed slices (5 rows) without pagination or “View all” links. |
| **Severity** | **Low** |
| **Proposed fix** | Add “View all targets/findings” links to filtered list routes, or paginate when >5 rows. |

### 5.4 Findings table lacks view mode

| | |
|---|---|
| **Issue** | Findings is table-only with pagination but no `ContentToolbar` / list alternative despite being a major data page. |
| **Severity** | **Medium** |
| **Proposed fix** | Add list view using `ListCard` (severity, title, project, timestamp) for parity with Scans/Targets. |

---

## 6. List Views

### 6.1 Three list implementations coexist

| | |
|---|---|
| **Issue** | **ListCard grid** (Projects, Targets), **scan-monitor-grid** (Scans list mode), **discovery-tree** hierarchy (Discovery list mode), **discovery-card** (Attacks), **model-card** (Models). Only Projects/Targets use the shared `ListCard` component. |
| **Severity** | **Critical** |
| **Proposed fix** | Define list-mode contract: `ListCard` for flat entities; `HierarchyList` wrapper for Discovery only (documented exception). Refactor Scans list to `ListCard` + optional embedded progress footer; refactor Attacks to `ListCard`. |

### 6.2 Scans list view ignores ListCard spec fields

| | |
|---|---|
| **Issue** | Iteration 5 list standard specifies Title, Status, Metadata, Timestamp, Actions. Scans list reuses `ScanMonitorCard` / `ScanHistoryCard` with different layout and typography. |
| **Severity** | **High** |
| **Proposed fix** | Wrap scan data in `ListCard`; move pause/resume/stop to `list-card__actions`. |

### 6.3 Discovery list mode has no pagination

| | |
|---|---|
| **Issue** | Table mode paginates; list mode renders full expandable tree with no page controls. Large workspaces will produce very long pages. |
| **Severity** | **High** |
| **Proposed fix** | Paginate at project level in tree mode, or collapse to ListCard flat rows sharing the same paginated dataset as table mode. |

### 6.4 Discovery list empty guidance contradicts table UX

| | |
|---|---|
| **Issue** | Tree list empty state says “Run discovery from table view” while table includes targets with no runs and per-row Run Discovery icons. |
| **Severity** | **Medium** |
| **Proposed fix** | Update copy; show targets without runs in tree mode with Run Discovery icon (mirror table rows). |

---

## 7. Pagination

### 7.1 Missing pagination on high-volume pages

| | |
|---|---|
| **Issue** | Attacks run history, Models catalog, Dashboard activity/projects snippets, and Discovery list mode have no pagination. |
| **Severity** | **High** (Attacks, Discovery list) / **Low** (Dashboard widgets) |
| **Proposed fix** | Add `Pagination` + `usePaginatedList` to Attacks. Dashboard widgets are summaries—keep unpaginated but link to full paginated pages. |

### 7.2 Separate page-size keys on Reports

| | |
|---|---|
| **Issue** | `reports-export` and `reports-archive` persist different page sizes. User may expect one global Reports preference. |
| **Severity** | **Low** |
| **Proposed fix** | Single `reports` page-size key unless sections are intentionally independent (document in Settings or UI). |

### 7.3 Pagination placement

| | |
|---|---|
| **Issue** | Pagination always appears below content. No top duplicate for long tables (accessibility convenience). |
| **Severity** | **Low** |
| **Proposed fix** | Optional sticky pagination bar or duplicate controls above table when `totalItems > pageSize`. |

### 7.4 Projects list empty skips pagination block correctly but table empty still shows toolbar

| | |
|---|---|
| **Issue** | When zero projects, `ContentToolbar` still renders above empty table/list. View toggle visible with nothing to toggle. |
| **Severity** | **Low** |
| **Proposed fix** | Hide `ContentToolbar` when `filtered.length === 0` and not loading. |

---

## 8. Empty States

### 8.1 Missing CSS for EmptyState sub-elements

| | |
|---|---|
| **Issue** | `EmptyState` renders `empty-state__description` and `empty-state__action` classes, but `global.css` only defines `empty-state`, `__icon`, and `__title`. Description/action spacing and typography are unstyled. |
| **Severity** | **High** |
| **Proposed fix** | Add CSS tokens for `empty-state__description` (muted, max-width) and `empty-state__action` (top margin). |

### 8.2 Targets page empty bypasses EmptyState

| | |
|---|---|
| **Issue** | Targets uses `<Card><p className="text-muted">…</p></Card>` instead of `EmptyState` with icon and optional “Add Target” CTA. |
| **Severity** | **Medium** |
| **Proposed fix** | Replace with `EmptyState` + primary Add Target action (matches Scans empty pattern). |

### 8.3 Reports stored section empty bypasses EmptyState

| | |
|---|---|
| **Issue** | Export section uses `EmptyState`; stored reports section uses plain muted paragraph. |
| **Severity** | **Medium** |
| **Proposed fix** | Use `EmptyState` consistently in both Reports sections. |

### 8.4 Loading text used as EmptyState title

| | |
|---|---|
| **Issue** | Attacks and Discovery empty checks use `title={loading ? "Loading…" : "No … yet"}`. EmptyState is not a loading indicator. |
| **Severity** | **Medium** |
| **Proposed fix** | Show skeleton/spinner while loading; show EmptyState only when `!loading && items.length === 0`. |

### 8.5 Projects list-mode empty inside grid

| | |
|---|---|
| **Issue** | List view empty renders a nested `Card` with paragraph inside `list-card-grid` instead of full-width `EmptyState`. |
| **Severity** | **Low** |
| **Proposed fix** | Render single `EmptyState` spanning grid (same as Scans `EmptyState` full-page pattern). |

---

## 9. Loading States

### 9.1 Four loading patterns in use

| | |
|---|---|
| **Issue** | (1) Route `PageLoader` spinner, (2) detail `PageHeader` + “Loading…” description, (3) inline `page-loader__spinner` in Card (Discovery/Attacks progress), (4) Refresh button text change. No shared `LoadingState` component. |
| **Severity** | **High** |
| **Proposed fix** | Introduce `LoadingState`/`PageSkeleton` component; use on detail pages instead of text-only headers. |

### 9.2 Store loading does not block table render

| | |
|---|---|
| **Issue** | Most list pages render tables with stale/empty data while `loading === true`, relying on `emptyMessage` strings. Can flash empty state before data arrives. |
| **Severity** | **Medium** |
| **Proposed fix** | When `loading && items.length === 0`, show skeleton rows or centered spinner instead of empty message. |

### 9.3 Scan Wizard step loading

| | |
|---|---|
| **Issue** | Project step shows inline “Loading project…” text; other steps mix disabled controls and spinners inconsistently. |
| **Severity** | **Low** |
| **Proposed fix** | Unified step-level loading overlay inside `wizard-panel__body`. |

---

## 10. Error States

### 10.1 Error message prefix inconsistency

| | |
|---|---|
| **Issue** | Some pages prefix errors (“Failed to load projects: …”, “Failed to load targets: …”); others show raw `{error}` (Findings, Reports, Scans). |
| **Severity** | **Medium** |
| **Proposed fix** | Standardize via `ErrorBanner` component with page context label and optional retry action. |

### 10.2 Fatal vs non-fatal error presentation

| | |
|---|---|
| **Issue** | Scan Details shows `EmptyState` for fatal load failure but inline `Card` error when partial data exists. Discovery Details only uses inline Card. |
| **Severity** | **Medium** |
| **Proposed fix** | Define rules: fatal → `EmptyState` + back; recoverable → dismissible `ErrorBanner` above content. |

### 10.3 No error handling on Dashboard, Models, Settings

| | |
|---|---|
| **Issue** | These pages read store data but do not surface `error` from `useAppStore()` if refresh fails. |
| **Severity** | **Medium** |
| **Proposed fix** | Add shared error banner when store `error` is set, consistent with list pages. |

### 10.4 Dashboard StatCard hints are hardcoded

| | |
|---|---|
| **Issue** | Hints like “2 active”, “1 scanning”, “1 downloading” are static strings, not derived from `stats`. Misleading when real counts differ. |
| **Severity** | **High** |
| **Proposed fix** | Compute hints from store stats or remove hints until dynamic values exist. |

### 10.5 Delete project lacks confirmation

| | |
|---|---|
| **Issue** | Delete from Projects table/list and Project Details dropdown executes immediately without confirm dialog. High-impact destructive action. |
| **Severity** | **High** |
| **Proposed fix** | Add shared `ConfirmDialog` before all delete operations. |

---

## 11. Cross-Cutting Recommendations

### Priority 1 (next sprint)

1. Add missing `EmptyState` CSS and unify empty/loading semantics (no loading titles in EmptyState).  
2. Standardize Refresh button (`secondary`, loading disabled, optional label).  
3. Add `backTo` on all detail loading/error/not-found views.  
4. Refactor Scans list mode to `ListCard`; paginate Discovery list mode.  
5. Replace hardcoded Dashboard stat hints with computed values.  
6. Add delete confirmation modal.

### Priority 2

1. Migrate Attacks page to contextual row actions; remove header launch cluster.  
2. Introduce `ErrorBanner` and `LoadingState` shared components.  
3. Extend `ActionsDropdown` with visible label; use for row overflow menus.  
4. Add Findings list view + `ContentToolbar`.  
5. Align Targets/Reports empty states with `EmptyState`.

### Priority 3 (polish)

1. Resolve TopBar vs PageHeader title duplication.  
2. Distinct nav icons for Scans vs Discovery.  
3. Scan Wizard header back to `/scans`.  
4. Table action icon set (view, delete, run).  
5. Hide view toggle when no data.

---

## Appendix: Shared Components Inventoried

| Component | Path | Adoption |
|-----------|------|----------|
| `PageHeader` (+ `backTo`) | `shared/components/PageHeader.tsx` | All pages; back on 4 detail routes |
| `ViewModeToggle` | `shared/components/ViewModeToggle.tsx` | Projects, Scans, Targets, Discovery |
| `ContentToolbar` | `shared/components/Pagination.tsx` | Same four pages |
| `Pagination` | `shared/components/Pagination.tsx` | Projects, Scans, Targets, Discovery (table), Findings, Reports |
| `ListCard` | `shared/components/ListCard.tsx` | Projects, Targets only |
| `ActionsDropdown` | `shared/components/ActionsDropdown.tsx` | Project Details only |
| `EmptyState` | `shared/components/EmptyState.tsx` | Most data pages; incomplete CSS |
| `Select` | `shared/components/Select.tsx` | Settings, Findings, Wizard, Attacks, Discovery (removed), Add Target |
| `IconButton` / `Icons` | `shared/components/` | PageHeader back, Discovery run action, ViewModeToggle |

---

## Document History

| Version | Date | Author | Notes |
|---------|------|--------|-------|
| 1.0 | 2026-06-13 | UX audit (automated) | Initial full consistency review post Iteration 5 |
