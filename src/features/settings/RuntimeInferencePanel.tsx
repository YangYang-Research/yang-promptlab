import { useMemo, useState } from "react";

import { Card } from "@/shared/components";
import { IconAi } from "@/shared/components/Icons";
import { paginateItems } from "@/shared/utils/pagination";

import { RUNTIME_INFERENCE_SITES } from "./runtimeInferenceSites";

const PAGE_SIZE = 4;

export function RuntimeInferencePanel() {
  const [page, setPage] = useState(1);
  const pagination = useMemo(
    () => paginateItems(RUNTIME_INFERENCE_SITES, page, PAGE_SIZE),
    [page],
  );

  return (
    <Card>
      <ul className="ai-applied-list">
        {pagination.items.map((site) => (
          <li key={site.id} className="ai-applied-list__item">
            <span className="ai-applied-list__icon-wrap" aria-hidden="true">
              <IconAi className="ai-applied-list__icon" />
            </span>
            <div className="ai-applied-list__body">
              <div className="ai-applied-list__header">
                <strong className="ai-applied-list__title">{site.title}</strong>
                <span className="ai-applied-list__location">{site.location}</span>
              </div>
              <p className="ai-applied-list__description text-muted text-sm">{site.description}</p>
            </div>
          </li>
        ))}
      </ul>
      {pagination.totalPages > 1 ? (
        <div className="ai-applied-list__pager">
          <button
            type="button"
            className="ai-applied-list__pager-btn"
            disabled={pagination.page <= 1}
            onClick={() => setPage((current) => Math.max(1, current - 1))}
          >
            Prev
          </button>
          <span className="ai-applied-list__pager-meta">
            {pagination.page}/{pagination.totalPages}
          </span>
          <button
            type="button"
            className="ai-applied-list__pager-btn"
            disabled={pagination.page >= pagination.totalPages}
            onClick={() =>
              setPage((current) => Math.min(pagination.totalPages, current + 1))
            }
          >
            Next
          </button>
        </div>
      ) : null}
    </Card>
  );
}
