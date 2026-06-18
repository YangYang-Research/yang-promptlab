//! OS-backed secret storage and encrypted session artifact vault.

mod store;
mod vault;
pub mod migrate;
pub mod audit;
mod descriptor;

pub use store::{CredentialReferenceId, SecretScope, SecretStore};
pub use vault::EncryptedVault;
pub use migrate::{
    migrate_legacy_auth_data, migrate_legacy_storage_artifacts, migrate_legacy_target_descriptors,
    profile_config_has_plaintext, resolve_auth_config_secrets, run_database_secret_migration,
    session_secrets_from_json, session_secrets_to_json, store_auth_config_secrets,
    SecretMigrationResult,
};
pub use audit::{
    audit_database_secrets, merge_judge_config_audit, SecretAuditItem, SecretMigrationAudit,
};
pub use descriptor::{
    descriptor_has_plaintext_secrets, resolve_descriptor_for_runtime, sanitize_target_descriptor,
};
