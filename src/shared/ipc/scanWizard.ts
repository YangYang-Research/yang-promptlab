import { invokeCommand } from "./invoke";
import type { ScanDto } from "./client";

export type ScanWizardLoadDto = {
  scan: ScanDto;
  wizard: unknown;
};

export type ScanWizardCreateRequest = {
  projectId: string;
  targetId?: string | null;
  wizard: unknown;
};

export type ScanWizardSaveRequest = {
  scanId: string;
  projectId: string;
  targetId?: string | null;
  wizard: unknown;
};

export const createWizardScan = (request: ScanWizardCreateRequest) =>
  invokeCommand<ScanDto>("scan_wizard_create", { request });

export const saveWizardScan = (request: ScanWizardSaveRequest) =>
  invokeCommand<ScanDto>("scan_wizard_save", { request });

export const loadWizardScan = (scanId: string) =>
  invokeCommand<ScanWizardLoadDto>("scan_wizard_load", { scanId });

export const WIZARD_SCAN_STATUS = "draft";
export const WIZARD_SCAN_NAME_PREFIX = "Setup Scan";

export function isWizardDraftScan(scan: { status: string; name: string }): boolean {
  return scan.status === WIZARD_SCAN_STATUS || scan.name.startsWith(WIZARD_SCAN_NAME_PREFIX);
}
