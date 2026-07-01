import { Badge } from "@/shared/components";

import type { AttackCategoryId } from "../attackProfiles";
import { getCategory } from "../attackProfiles";
import {
  resolveAttackGraphStates,
  type AttackGraphNodeState,
} from "../attackGraphProgress";
import type { ScanStatusDto } from "@/shared/ipc";

type AttackGraphProgressProps = {
  categories: AttackCategoryId[];
  status: ScanStatusDto | null;
  compact?: boolean;
};

export function AttackGraphProgress({
  categories,
  status,
  compact = false,
}: AttackGraphProgressProps) {
  const states = resolveAttackGraphStates(categories, status);

  if (categories.length === 0) {
    return <p className="text-sm text-muted">No attack categories selected.</p>;
  }

  return (
    <ol className={`wizard-attack-graph${compact ? " wizard-attack-graph--compact" : ""}`}>
      {categories.map((category, index) => {
        const state = states.get(category) ?? "pending";
        const meta = getCategory(category);
        return (
          <li
            key={category}
            className={`wizard-attack-graph__node wizard-attack-graph__node--${state}`}
          >
            <span className="wizard-attack-graph__index">{index + 1}</span>
            <div className="wizard-attack-graph__body">
              <div className="wizard-attack-graph__title-row">
                <span className="wizard-attack-graph__label">{meta.label}</span>
                <Badge variant={badgeVariant(state)}>{stateLabel(state)}</Badge>
              </div>
              {!compact && (
                <p className="wizard-attack-graph__description text-sm text-muted">
                  {meta.description}
                </p>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

function stateLabel(state: AttackGraphNodeState): string {
  switch (state) {
    case "active":
      return "Attacking";
    case "done":
      return "Done";
    case "failed":
      return "Failed";
    default:
      return "Pending";
  }
}

function badgeVariant(
  state: AttackGraphNodeState,
): "success" | "warning" | "danger" | "info" | "muted" {
  if (state === "active") return "info";
  if (state === "done") return "success";
  if (state === "failed") return "danger";
  return "muted";
}
