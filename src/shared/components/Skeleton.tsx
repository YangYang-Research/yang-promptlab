type SkeletonProps = {
  className?: string;
  width?: string;
  height?: string;
  rounded?: "sm" | "md" | "full";
};

export function Skeleton({
  className = "",
  width,
  height = "1rem",
  rounded = "sm",
}: SkeletonProps) {
  return (
    <span
      className={`skeleton skeleton--${rounded} ${className}`.trim()}
      style={{ width, height }}
      aria-hidden="true"
    />
  );
}

export function PageLoadingSkeleton() {
  return (
    <div className="page-loading-skeleton" aria-busy="true" aria-label="Loading page">
      <div className="page-loading-skeleton__header">
        <Skeleton width="12rem" height="1.75rem" rounded="sm" />
        <Skeleton width="20rem" height="0.875rem" rounded="sm" />
      </div>
      <div className="page-loading-skeleton__stats">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="page-loading-skeleton__stat card card--pad-md">
            <Skeleton width="5rem" height="0.75rem" />
            <Skeleton width="3rem" height="1.75rem" className="page-loading-skeleton__stat-value" />
            <Skeleton width="6rem" height="0.625rem" />
          </div>
        ))}
      </div>
      <div className="page-loading-skeleton__body card card--pad-md">
        <Skeleton width="40%" height="0.875rem" />
        <Skeleton width="100%" height="2.5rem" />
        <Skeleton width="100%" height="2.5rem" />
        <Skeleton width="85%" height="2.5rem" />
      </div>
    </div>
  );
}
