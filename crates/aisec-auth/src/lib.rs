//! AISec Authentication Engine — Playwright-backed login recording and session management.

pub mod config;
pub mod cookies;
pub mod engine;
pub mod mock;
pub mod paths;
pub mod playwright;
pub mod secrets;
pub mod session;
pub mod types;

pub use config::AuthEngineConfig;
pub use cookies::{CookieManager, TokenExtractor};
pub use engine::AuthEngine;
pub use mock::{MockPlaywrightDriver, SharedPlaywrightDriver};
pub use paths::{auth_sessions_dir, default_data_root};
pub use playwright::{PlaywrightClient, PlaywrightDriver};
pub use secrets::{
    audit_database_secrets, descriptor_has_plaintext_secrets, merge_judge_config_audit,
    migrate_legacy_auth_data, migrate_legacy_storage_artifacts, migrate_legacy_target_descriptors,
    profile_config_has_plaintext, resolve_descriptor_for_runtime, resolve_descriptor_for_wizard,
    run_database_secret_migration,
    sanitize_target_descriptor, CredentialReferenceId, EncryptedVault, ModelCredentialVault,
    SecretAuditItem, SecretMigrationAudit, SecretMigrationResult, SecretScope, SecretStore,
};
pub use session::{AuthSessionManager, SessionAuthContext, SessionStore};
pub use types::*;
