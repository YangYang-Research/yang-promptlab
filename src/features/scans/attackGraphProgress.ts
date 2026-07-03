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

  const terminal = ["completed", "failed", "cancelled", "stopped"].includes(status.status);
  const categoriesCompleted = status.categories_completed ?? 0;
  const activeId = categoryIdFromCurrentTest(status.current_test);
  const activeIndex = activeId ? categories.indexOf(activeId) : -1;
  const phase = status.current_phase?.toLowerCase() ?? null;

  categories.forEach((category, index) => {
    if (terminal && status.status === "completed") {
      states.set(category, "done");
      return;
    }
    if (terminal && status.status === "failed" && index === categoriesCompleted) {
      states.set(category, "failed");
      return;
    }
    if (index < categoriesCompleted) {
      states.set(category, "done");
      return;
    }
    if (!terminal && index === categoriesCompleted) {
      if (activeIndex === index || activeIndex < 0) {
        states.set(category, "active");
        return;
      }
    }
    if (!terminal && activeIndex === index) {
      states.set(category, "active");
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

export function attackGraphStateLabel(
  state: AttackGraphNodeState,
  status: ScanStatusDto | null,
  categoryId: AttackCategoryId,
): string {
  if (state !== "active" || !status) {
    return defaultStateLabel(state);
  }
  const phase = status.current_phase?.toLowerCase() ?? null;
  const activeId = categoryIdFromCurrentTest(status.current_test);
  if (activeId !== categoryId) {
    return defaultStateLabel(state);
  }
  if (phase === "judge") return "Judging";
  if (phase === "attack") return "Attacking";
  return "Running";
}

function defaultStateLabel(state: AttackGraphNodeState): string {
  switch (state) {
    case "active":
      return "Running";
    case "done":
      return "Done";
    case "failed":
      return "Failed";
    default:
      return "Pending";
  }
}
