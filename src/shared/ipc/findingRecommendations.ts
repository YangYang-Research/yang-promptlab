import { invokeCommand } from "./invoke";

export type FindingRecommendationDto = {
  title: string;
  description: string;
  priority: string;
};

export type FindingRecommendationsResponse = {
  source: "ai" | "fallback" | string;
  overview: string;
  recommendations: FindingRecommendationDto[];
  generated_at: string;
};

export function generateFindingRecommendations(
  findingId: string,
  force = false,
): Promise<FindingRecommendationsResponse> {
  return invokeCommand<FindingRecommendationsResponse>("finding_recommendations_generate", {
    request: {
      finding_id: findingId,
      force,
    },
  });
}
