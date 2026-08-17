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

/** Max in-flight finding-remediation LLM calls (Report Details stampede guard). */
const FINDING_RECOMMEND_MAX_CONCURRENT = 3;

type FindingRecommendJob = {
  findingId: string;
  force: boolean;
  /** Lower runs first — Report Details uses list index (top → bottom). */
  order: number;
  /** Stable tie-break when multiple jobs share the same order. */
  seq: number;
  resolve: (value: FindingRecommendationsResponse) => void;
  reject: (reason: unknown) => void;
};

const pending: FindingRecommendJob[] = [];
let active = 0;
let enqueueSeq = 0;

function compareJobs(a: FindingRecommendJob, b: FindingRecommendJob): number {
  if (a.order !== b.order) return a.order - b.order;
  return a.seq - b.seq;
}

function pumpFindingRecommendQueue(): void {
  while (active < FINDING_RECOMMEND_MAX_CONCURRENT && pending.length > 0) {
    pending.sort(compareJobs);
    const job = pending.shift();
    if (!job) return;

    active += 1;
    void invokeCommand<FindingRecommendationsResponse>("finding_recommendations_generate", {
      request: {
        finding_id: job.findingId,
        force: job.force,
      },
    })
      .then(job.resolve, job.reject)
      .finally(() => {
        active -= 1;
        pumpFindingRecommendQueue();
      });
  }
}

export type GenerateFindingRecommendationsOptions = {
  force?: boolean;
  /**
   * Document order for Report Details (0 = top). Lower values start first.
   * Forced regenerations use a boosted order so they are not starved.
   */
  order?: number;
};

/**
 * Queue finding-remediation IPC calls: top→bottom order, max 3 concurrent.
 */
export function generateFindingRecommendations(
  findingId: string,
  forceOrOptions: boolean | GenerateFindingRecommendationsOptions = false,
): Promise<FindingRecommendationsResponse> {
  const options =
    typeof forceOrOptions === "boolean"
      ? { force: forceOrOptions }
      : forceOrOptions;
  const force = options.force ?? false;
  const order = force ? -1 : (options.order ?? 0);

  return new Promise((resolve, reject) => {
    pending.push({
      findingId,
      force,
      order,
      seq: enqueueSeq++,
      resolve,
      reject,
    });
    pumpFindingRecommendQueue();
  });
}
