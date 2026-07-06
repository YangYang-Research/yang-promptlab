import { invokeCommand } from "./invoke";

export type AttackRecommendationDto = {
  title: string;
  description: string;
  priority: string;
};

export type ScanRecommendationsResponse = {
  source: "ai" | "fallback";
  recommendations: AttackRecommendationDto[];
};

export function generateScanRecommendations(
  scanId: string,
  attackCategories: string[] = [],
): Promise<ScanRecommendationsResponse> {
  return invokeCommand<ScanRecommendationsResponse>("scan_recommendations_generate", {
    request: {
      scan_id: scanId,
      attack_categories: attackCategories,
    },
  });
}
