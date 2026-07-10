import { invokeCommand } from "./invoke";

export type AttackCatalogTechniqueDto = {
  id: string;
  categoryId: string;
  name: string;
  description: string | null;
  content: string;
  defaultContent: string;
  tags: string[];
  surface: string | null;
  owasp: string | null;
  enabled: boolean;
  userModified: boolean;
  sortOrder: number;
};

export type AttackCatalogCategoryDto = {
  id: string;
  label: string;
  techniqueCount: number;
  enabledCount: number;
};

export type UpdateAttackCatalogTechniqueRequest = {
  content?: string;
  enabled?: boolean;
  name?: string;
  description?: string;
};

export function listAttackCatalog(): Promise<AttackCatalogTechniqueDto[]> {
  return invokeCommand<AttackCatalogTechniqueDto[]>("attack_catalog_list");
}

export function listAttackCatalogCategories(): Promise<AttackCatalogCategoryDto[]> {
  return invokeCommand<AttackCatalogCategoryDto[]>("attack_catalog_categories");
}

export function updateAttackCatalogTechnique(
  id: string,
  request: UpdateAttackCatalogTechniqueRequest,
): Promise<AttackCatalogTechniqueDto> {
  return invokeCommand<AttackCatalogTechniqueDto>("attack_catalog_update", { id, request });
}

export function resetAttackCatalogTechnique(id: string): Promise<AttackCatalogTechniqueDto> {
  return invokeCommand<AttackCatalogTechniqueDto>("attack_catalog_reset", { id });
}
