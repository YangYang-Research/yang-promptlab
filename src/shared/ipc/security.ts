import { invokeCommand } from "./invoke";

export type SecretAuditItem = {
  area: string;
  recordId: string;
  field: string;
  message: string;
};

export type SecretMigrationAudit = {
  legacyCount: number;
  targetsLegacy: number;
  authProfilesLegacy: number;
  sessionsLegacy: number;
  sessionStorageLegacy: number;
  judgeConfigLegacy: number;
  items: SecretAuditItem[];
};

export type SecretMigrationReport = {
  auditBefore: SecretMigrationAudit;
  auditAfter: SecretMigrationAudit;
  authMigrated: number;
  targetsMigrated: number;
  storageMigrated: number;
  judgeMigrated: number;
};

export function securityAudit(): Promise<SecretMigrationAudit> {
  return invokeCommand<SecretMigrationAudit>("security_audit");
}

export function securityMigrateSecrets(): Promise<SecretMigrationReport> {
  return invokeCommand<SecretMigrationReport>("security_migrate_secrets");
}
