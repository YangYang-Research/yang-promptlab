import { useEffect, useMemo, useState } from "react";

import type { PageSize } from "@/shared/hooks/usePageSizePreference";
import { paginateItems } from "@/shared/utils/pagination";

export function usePaginatedList<T>(items: T[], pageSize: PageSize) {
  const [page, setPage] = useState(1);

  const pagination = useMemo(
    () => paginateItems(items, page, pageSize),
    [items, page, pageSize],
  );

  useEffect(() => {
    setPage(1);
  }, [items.length, pageSize]);

  useEffect(() => {
    if (page > pagination.totalPages) {
      setPage(pagination.totalPages);
    }
  }, [page, pagination.totalPages]);

  return {
    page,
    setPage,
    pagination,
  };
}
