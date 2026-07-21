import { emitLiveLog, type LiveLogCategory, type LiveLogSeverity } from "@/shared/ipc/environment";

type WizardLogInput = {
  category?: LiveLogCategory;
  severity?: LiveLogSeverity;
  activityName: string;
  message: string;
  projectId?: string | null;
  scanId?: string | null;
  attributes?: Record<string, unknown>;
  component?: string;
};

/** Scan wizard → Live logs (OCSF / Troubleshooting). */
export function logWizardEvent(input: WizardLogInput): void {
  emitLiveLog({
    category: input.category ?? "scan",
    severity: input.severity,
    activityName: input.activityName,
    message: input.message,
    module: "scan-wizard",
    component: input.component ?? "ScanWizardPage",
    projectId: input.projectId,
    scanId: input.scanId,
    attributes: input.attributes,
  });
}
