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
  generated_at: string;
};

export function generateScanRecommendations(
  scanId: string,
  attackCategories: string[] = [],
  force = false,
): Promise<ScanRecommendationsResponse> {
  return invokeCommand<ScanRecommendationsResponse>("scan_recommendations_generate", {
    request: {
      scan_id: scanId,
      attack_categories: attackCategories,
      force,
    },
  });
}
