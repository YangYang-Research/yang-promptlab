import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";

import { IconBack } from "./Icons";
import { IconButton } from "./IconButton";

type PageHeaderProps = {
  title: string;
  description?: ReactNode;
  backTo?: string;
  /** Compact header: back icon only (title available to screen readers). */
  backOnly?: boolean;
  actions?: ReactNode;
};

export function PageHeader({
  title,
  description,
  backTo,
  backOnly = false,
  actions,
}: PageHeaderProps) {
  const navigate = useNavigate();
  const compact = backOnly && Boolean(backTo);

  return (
    <header className={`page-header ${compact ? "page-header--compact" : ""}`}>
      <div className="page-header__text">
        {backTo ? (
          <div className="page-header__title-row">
            <IconButton ariaLabel={`Go back from ${title}`} onClick={() => navigate(backTo)}>
              <IconBack />
            </IconButton>
            {!compact && (
              <>
                <h1 className="page-header__title">{title}</h1>
                {description && <p className="page-header__description">{description}</p>}
              </>
            )}
          </div>
        ) : (
          <>
            <h1 className="page-header__title">{title}</h1>
            {description && <p className="page-header__description">{description}</p>}
          </>
        )}
        {compact && <h1 className="sr-only">{title}</h1>}
      </div>
      {actions && <div className="page-header__actions">{actions}</div>}
    </header>
  );
}
