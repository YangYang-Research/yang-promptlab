export type SetupStepId = "add-model" | "load-model" | "first-project-scan";

export type SetupStep = {
  id: SetupStepId;
  title: string;
  description: string;
  to: string;
  /** Optional react-router location state (e.g. open create-project modal). */
  linkState?: Record<string, unknown>;
  done: boolean;
  /** True when a prior step is still incomplete — not clickable yet. */
  locked: boolean;
};

export type SetupProgressInput = {
  mode: "not_configured" | "third_party" | "local";
  initialized: boolean;
  localModelCount: number;
  configuredThirdPartyCount: number;
  selectedModelId: string | null;
  modelLoaded: boolean;
  projectCount: number;
  scanCount: number;
};

const STEP_META: Array<{
  id: SetupStepId;
  title: string;
  description: string;
  to: string;
}> = [
  {
    id: "add-model",
    title: "Register a remote model",
    description: "Add an OpenAI-compatible or cloud provider model for AI Runtime.",
    to: "/models",
  },
  {
    id: "load-model",
    title: "Choose a model for AI Runtime",
    description: "Pick which registered remote model AI Runtime should use.",
    to: "/runtime",
  },
  {
    id: "first-project-scan",
    title: "Let's start",
    description: "Create a project, then start your first scan.",
    to: "/scans/new",
  },
];

export function deriveSetupSteps(input: SetupProgressInput): SetupStep[] {
  const hasRemoteModel = input.configuredThirdPartyCount > 0 || input.localModelCount > 0;
  const modelSelected =
    Boolean(input.selectedModelId) && input.configuredThirdPartyCount > 0;

  const rawDone = [
    hasRemoteModel,
    modelSelected,
    input.projectCount > 0 && input.scanCount > 0,
  ];

  // Enforce strict order: a step can only be done if every prior step is done.
  const sequentialDone: boolean[] = [];
  for (let i = 0; i < rawDone.length; i += 1) {
    sequentialDone[i] = Boolean(rawDone[i] && (i === 0 || sequentialDone[i - 1]));
  }

  return STEP_META.map((meta, index) => {
    const done = sequentialDone[index] ?? false;
    const locked = index > 0 && !(sequentialDone[index - 1] ?? false);
    if (meta.id === "first-project-scan") {
      const hasProject = input.projectCount > 0;
      return {
        ...meta,
        to: hasProject ? "/scans/new" : "/projects",
        linkState: hasProject ? undefined : { openNewProject: true },
        done,
        locked,
      };
    }
    return { ...meta, done, locked };
  });
}

export function setupProgress(steps: SetupStep[]): {
  doneCount: number;
  total: number;
  allDone: boolean;
} {
  const doneCount = steps.filter((step) => step.done).length;
  return {
    doneCount,
    total: steps.length,
    allDone: doneCount === steps.length && steps.length > 0,
  };
}
