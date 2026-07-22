import { invokeCommand } from "./invoke";

export type AttackRecommendationDto = {
  title: string;
  description: string;
  priority: string;
  /** Optional UI action: `retry_scan` | `start_attack`. */
  action?: string | null;
};

export type ScanRecommendationsResponse = {
  source: "ai" | "fallback";
  overview: string;
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
