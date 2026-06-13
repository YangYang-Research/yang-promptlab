//! OS-backed secret storage and encrypted session artifact vault.

mod store;
mod vault;
pub(crate) mod migrate;
mod descriptor;

pub use store::{CredentialReferenceId, SecretScope, SecretStore};
pub use vault::EncryptedVault;
pub use migrate::{
    migrate_legacy_auth_data, migrate_legacy_storage_artifacts, migrate_legacy_target_descriptors,
    resolve_auth_config_secrets, session_secrets_from_json, session_secrets_to_json,
    store_auth_config_secrets,
};
pub use descriptor::{resolve_descriptor_for_runtime, sanitize_target_descriptor};
