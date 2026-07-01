import type { AttackCategoryId } from "./attackProfiles";
import { ATTACK_CATALOG } from "./attackProfiles";
import type { ScanStatusDto } from "@/shared/ipc";

export type AttackGraphNodeState = "pending" | "active" | "done" | "failed";

export function categoryIdFromCurrentTest(
  currentTest: string | null | undefined,
): AttackCategoryId | null {
  if (!currentTest?.trim()) return null;
  const normalized = currentTest.trim().toLowerCase();
  const match = ATTACK_CATALOG.find(
    (category) =>
      category.label.toLowerCase() === normalized ||
      category.id.replace(/_/g, " ") === normalized,
  );
  return match?.id ?? null;
}

export function resolveAttackGraphStates(
  categories: AttackCategoryId[],
  status: ScanStatusDto | null,
): Map<AttackCategoryId, AttackGraphNodeState> {
  const states = new Map<AttackCategoryId, AttackGraphNodeState>();
  for (const category of categories) {
    states.set(category, "pending");
  }
  if (!status || categories.length === 0) {
    return states;
  }

  const completed = Math.min(status.completed, categories.length);
  const activeId = categoryIdFromCurrentTest(status.current_test);
  const terminal = ["completed", "failed", "cancelled", "stopped"].includes(status.status);

  categories.forEach((category, index) => {
    if (index < completed) {
      states.set(category, "done");
      return;
    }
    if (!terminal && category === activeId) {
      states.set(category, "active");
      return;
    }
    if (!terminal && !activeId && index === completed) {
      states.set(category, "active");
      return;
    }
    if (terminal && status.status === "failed" && index === completed) {
      states.set(category, "failed");
      return;
    }
    if (terminal && status.status === "completed" && index < categories.length) {
      states.set(category, "done");
      return;
    }
    states.set(category, "pending");
  });

  if (terminal && status.status === "completed") {
    for (const category of categories) {
      states.set(category, "done");
    }
  }

  return states;
}
