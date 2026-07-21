import { getCategory, type AttackCategoryId } from "@/features/scans/attackProfiles";

export function categoryLabel(categoryId: string): string {
  try {
    return getCategory(categoryId as AttackCategoryId).label;
  } catch {
    return categoryId.replace(/_/g, " ");
  }
}
