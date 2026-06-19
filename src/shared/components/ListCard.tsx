import type { ReactNode } from "react";

type ListCardProps = {
  title: ReactNode;
  status?: ReactNode;
  metadata: Array<{ label: string; value: ReactNode }>;
  timestamp?: ReactNode;
  footerMeta?: ReactNode;
  actions?: ReactNode;
  onClick?: () => void;
};

export function ListCard({
  title,
  status,
  metadata,
  timestamp,
  footerMeta,
  actions,
  onClick,
}: ListCardProps) {
  const clickable = Boolean(onClick);

  return (
    <article
      className={`list-card ${clickable ? "list-card--clickable" : ""}`}
      onClick={onClick}
      onKeyDown={
        clickable
          ? (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onClick?.();
              }
            }
          : undefined
      }
      role={clickable ? "button" : undefined}
      tabIndex={clickable ? 0 : undefined}
    >
      <div className="list-card__header">
        <div className="list-card__title-wrap">
          <h3 className="list-card__title">{title}</h3>
          {timestamp && <div className="list-card__timestamp text-muted text-sm">{timestamp}</div>}
        </div>
        {status}
      </div>

      <dl className="list-card__metadata">
        {metadata.map((item) => (
          <div key={item.label}>
            <dt>{item.label}</dt>
            <dd>{item.value}</dd>
          </div>
        ))}
      </dl>

      {(footerMeta || actions) && (
        <div className="card-footer list-card__actions">
          {footerMeta && (
            <span className="card-footer-meta list-card__updated text-sm text-muted">{footerMeta}</span>
          )}
          {actions && (
            <div
              className="card-footer-actions"
              onClick={(event) => event.stopPropagation()}
              onKeyDown={(event) => event.stopPropagation()}
            >
              {actions}
            </div>
          )}
        </div>
      )}
    </article>
  );
}
