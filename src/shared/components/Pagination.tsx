import type { ReactNode } from "react";

import { PAGE_SIZE_OPTIONS, type PageSize } from "@/shared/hooks/usePageSizePreference";
import type { ViewMode } from "@/shared/hooks/useViewPreference";
import { pageNumbers } from "@/shared/utils/pagination";

import { Select } from "./Select";
import { ViewModeToggle } from "./ViewModeToggle";

type PageSizeSelectProps = {
  pageSize: PageSize;
  onPageSizeChange: (pageSize: PageSize) => void;
  showLabel?: boolean;
};

export function PageSizeSelect({
  pageSize,
  onPageSizeChange,
  showLabel = true,
}: PageSizeSelectProps) {
  return (
    <label className="page-size-select">
      {showLabel && <span className="page-size-select__label">Items per page</span>}
      <Select
        inline
        value={String(pageSize)}
        onChange={(event) => onPageSizeChange(Number(event.target.value) as PageSize)}
        aria-label="Items per page"
      >
        {PAGE_SIZE_OPTIONS.map((size) => (
          <option key={size} value={size}>
            {size}
          </option>
        ))}
      </Select>
    </label>
  );
}

type PaginationProps = {
  page: number;
  totalItems: number;
  rangeStart: number;
  rangeEnd: number;
  totalPages: number;
  onPageChange: (page: number) => void;
};

export function Pagination({
  page,
  totalItems,
  rangeStart,
  rangeEnd,
  totalPages,
  onPageChange,
}: PaginationProps) {
  const pages = pageNumbers(page, totalPages);

  return (
    <div className="pagination">
      <div className="pagination__meta">
        <span className="pagination__summary">
          Showing {rangeStart}-{rangeEnd} of {totalItems} items
        </span>
      </div>

      <div className="pagination__controls">
        <button
          type="button"
          className="pagination__button"
          disabled={page <= 1}
          onClick={() => onPageChange(page - 1)}
        >
          Previous
        </button>
        <div className="pagination__pages" role="group" aria-label="Pagination">
          {pages.map((pageNumber, index) => {
            const previous = pages[index - 1];
            const showEllipsis = previous !== undefined && pageNumber - previous > 1;
            return (
              <span key={pageNumber} className="pagination__page-group">
                {showEllipsis && <span className="pagination__ellipsis">…</span>}
                <button
                  type="button"
                  className={`pagination__page ${
                    pageNumber === page ? "pagination__page--active" : ""
                  }`}
                  aria-current={pageNumber === page ? "page" : undefined}
                  onClick={() => onPageChange(pageNumber)}
                >
                  {pageNumber}
                </button>
              </span>
            );
          })}
        </div>
        <button
          type="button"
          className="pagination__button"
          disabled={page >= totalPages}
          onClick={() => onPageChange(page + 1)}
        >
          Next
        </button>
      </div>
    </div>
  );
}

type ContentToolbarProps = {
  filters?: ReactNode;
  pageSize?: PageSize;
  onPageSizeChange?: (pageSize: PageSize) => void;
  viewMode?: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  showViewMode?: boolean;
  showPageSizeLabel?: boolean;
};

export function ContentToolbar({
  filters,
  pageSize,
  onPageSizeChange,
  viewMode,
  onViewModeChange,
  showViewMode = true,
  showPageSizeLabel = false,
}: ContentToolbarProps) {
  const hasPageSize = pageSize !== undefined && onPageSizeChange !== undefined;
  const hasViewMode = showViewMode && viewMode !== undefined && onViewModeChange !== undefined;

  if (!filters && !hasPageSize && !hasViewMode) return null;

  return (
    <div className="content-toolbar">
      <div className="content-toolbar__filters">{filters ?? null}</div>
      <div className="content-toolbar__controls">
        {hasPageSize && (
          <PageSizeSelect
            pageSize={pageSize}
            onPageSizeChange={onPageSizeChange}
            showLabel={showPageSizeLabel}
          />
        )}
        {hasViewMode && <ViewModeToggle mode={viewMode} onChange={onViewModeChange} />}
      </div>
    </div>
  );
}
