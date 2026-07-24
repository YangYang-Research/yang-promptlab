import { parseFindingEvidence } from "@/features/findings/findingEvidence";
import { ATTACK_CATALOG } from "@/features/scans/attackProfiles";
import type { Finding, Severity } from "@/shared/types";

export type SeveritySubcategoryRow = {
  label: string;
  count: number;
};

export type SeverityCategoryBreakdown = {
  categoryId: string;
  categoryLabel: string;
  totalCount: number;
  subcategories: SeveritySubcategoryRow[];
};

const testNameById = new Map(
  ATTACK_CATALOG.flatMap((category) => category.tests.map((test) => [test.id, test.name] as const)),
);

/** Mutated payloads use `{source_id}:{generation_uuid}` (see promptlab-generator convert). */
function baseTestIdFromPayloadId(payloadId: string): string {
  const colon = payloadId.indexOf(":");
  if (colon <= 0) return payloadId;
  const prefix = payloadId.slice(0, colon);
  if (testNameById.has(prefix)) return prefix;
  return payloadId;
}

export function resolveFindingSubcategory(finding: Pick<Finding, "title" | "evidence">): string {
  const evidence = parseFindingEvidence(finding);
  if (evidence.payloadId) {
    const baseId = baseTestIdFromPayloadId(evidence.payloadId);
    const fromCatalog = testNameById.get(baseId) ?? testNameById.get(evidence.payloadId);
    if (fromCatalog) return fromCatalog;
    return baseId.replace(/-/g, " ");
  }

  const colonIdx = finding.title.indexOf(": ");
  if (colonIdx >= 0) {
    const suffix = finding.title.slice(colonIdx + 2).trim();
    if (suffix) return suffix;
  }

  return finding.title;
}

export function buildSeverityBreakdown(
  findings: Finding[],
  severity: Severity,
  formatCategoryLabel: (categoryId: string) => string,
): SeverityCategoryBreakdown[] {
  const byCategory = new Map<string, Map<string, number>>();

  for (const finding of findings) {
    if (finding.severity !== severity) continue;
    const subcategory = resolveFindingSubcategory(finding);
    const subMap = byCategory.get(finding.category) ?? new Map<string, number>();
    subMap.set(subcategory, (subMap.get(subcategory) ?? 0) + 1);
    byCategory.set(finding.category, subMap);
  }

  return [...byCategory.entries()]
    .sort((a, b) => formatCategoryLabel(a[0]).localeCompare(formatCategoryLabel(b[0])))
    .map(([categoryId, subMap]) => {
      const subcategories = [...subMap.entries()]
        .sort((a, b) => a[0].localeCompare(b[0]))
        .map(([label, count]) => ({ label, count }));
      const totalCount = subcategories.reduce((sum, row) => sum + row.count, 0);
      return {
        categoryId,
        categoryLabel: formatCategoryLabel(categoryId),
        totalCount,
        subcategories,
      };
    });
}
