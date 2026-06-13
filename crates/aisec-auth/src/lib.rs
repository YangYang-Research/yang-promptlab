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
    migrate_legacy_auth_data, migrate_legacy_storage_artifacts, migrate_legacy_target_descriptors,
    resolve_descriptor_for_runtime, sanitize_target_descriptor, CredentialReferenceId,
    EncryptedVault, SecretScope, SecretStore,
};
pub use session::{AuthSessionManager, SessionAuthContext, SessionStore};
pub use types::*;
