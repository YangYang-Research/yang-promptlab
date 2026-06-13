import type { ViewMode } from "@/shared/hooks/useViewPreference";

import { IconList, IconTable } from "./Icons";
import { IconButton } from "./IconButton";

type ViewModeToggleProps = {
  mode: ViewMode;
  onChange: (mode: ViewMode) => void;
};

export function ViewModeToggle({ mode, onChange }: ViewModeToggleProps) {
  return (
    <div className="view-mode-toggle" role="group" aria-label="View mode">
      <IconButton
        ariaLabel="Table view"
        active={mode === "table"}
        onClick={() => onChange("table")}
      >
        <IconTable />
      </IconButton>
      <IconButton
        ariaLabel="List view"
        active={mode === "list"}
        onClick={() => onChange("list")}
      >
        <IconList />
      </IconButton>
    </div>
  );
}
