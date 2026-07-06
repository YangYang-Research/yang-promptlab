import { useEffect, useMemo, useState } from "react";

import { SeverityBadge } from "@/shared/components";
import type { ActivityItem } from "@/shared/types";

const MAX_ACTIVITY_PAGES = 2;
const ITEMS_PER_PAGE = 6;

function formatRelativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "Unknown time";

  const diff = Date.now() - then;
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;

  return new Date(iso).toLocaleDateString();
}

function formatAbsoluteTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "Unknown time";
  return date.toLocaleString();
}

type RecentActivityPanelProps = {
  activity: ActivityItem[];
};

export function RecentActivityPanel({ activity }: RecentActivityPanelProps) {
  const [page, setPage] = useState(0);

  const maxVisible = ITEMS_PER_PAGE * MAX_ACTIVITY_PAGES;
  const visibleActivity = useMemo(
    () => activity.slice(0, maxVisible),
    [activity, maxVisible],
  );

  const totalPages = Math.min(
    MAX_ACTIVITY_PAGES,
    Math.max(1, Math.ceil(visibleActivity.length / ITEMS_PER_PAGE)),
  );

  useEffect(() => {
    setPage((current) => Math.min(current, totalPages - 1));
  }, [totalPages]);

  const pageItems = visibleActivity.slice(
    page * ITEMS_PER_PAGE,
    page * ITEMS_PER_PAGE + ITEMS_PER_PAGE,
  );

  return (
    <div className="dashboard-activity">
      <h3 className="card__title">Recent Activity</h3>
      {visibleActivity.length === 0 ? (
        <p className="text-muted text-sm">No recent activity yet.</p>
      ) : (
        <>
          <ul className="activity-list activity-list--timeline activity-list--paged">
            {pageItems.map((item) => (
              <li key={item.id} className="activity-list__item">
                <span className={`activity-list__dot activity-list__dot--${item.type}`} />
                <div className="activity-list__body">
                  <p className="activity-list__message">{item.message}</p>
                  <time
                    className="activity-list__time"
                    dateTime={item.timestamp}
                    title={formatAbsoluteTime(item.timestamp)}
                  >
                    {formatRelativeTime(item.timestamp)}
                  </time>
                </div>
                {item.severity ? <SeverityBadge severity={item.severity} /> : null}
              </li>
            ))}
          </ul>
          {totalPages > 1 && (
            <div className="dashboard-activity__pagination">
              <button
                type="button"
                className="dashboard-activity__page-btn"
                disabled={page === 0}
                onClick={() => setPage((current) => Math.max(0, current - 1))}
              >
                Previous
              </button>
              <span className="dashboard-activity__page-label text-muted text-sm">
                Page {page + 1} of {totalPages}
              </span>
              <button
                type="button"
                className="dashboard-activity__page-btn"
                disabled={page >= totalPages - 1}
                onClick={() => setPage((current) => Math.min(totalPages - 1, current + 1))}
              >
                Next
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
