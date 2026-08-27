//! PromptLab core library — shared error handling, environment, OCSF logging, and domain primitives.

pub mod canary;
pub mod environment;
pub mod error;
pub mod event_log;
pub mod http;
pub mod logging;
pub mod proxy;

pub use canary::{
    ensure_in_content as ensure_canary_in_content, find_in as find_canary_in, mint as mint_canary,
    mint_for_category as mint_canary_for_category, mint_stable as mint_stable_canary,
    mint_stable_for_category as mint_stable_canary_for_category,
    response_contains as response_contains_canary, sanitize_payload_id, suite_for_category,
    CANARY_PLACEHOLDER,
};
pub use environment::{
    bootstrap_environment, ensure_environment, resolve_db_path, resolve_paths, user_home,
    EnvironmentConfig, EnvironmentPaths, DB_FILENAME, DB_PATH_ENV, ROOT_DIR_NAME, ROOT_PATH_ENV,
};
pub use error::{ErrorCode, PromptLabError, PromptLabResult};
pub use event_log::{
    global_event_bus, global_event_ring, list_log_files, mask_secrets, publish_crash,
    read_log_tail, spawn_event_logger, EventBus, EventLogGuard, EventRing, LogCategory,
    OcsfEvent, OcsfSeverity,
};
pub use http::{
    apply_proxy_settings, build_http_client, build_http_client_with, default_http_client,
    HttpClientOptions,
};
pub use logging::{init_logging, LogGuard, LogOptions};
pub use proxy::{
    bootstrap_proxy_settings, current_proxy_settings, install_proxy_settings, load_proxy_settings,
    proxy_settings_path, save_proxy_settings, ProxySettings, DEFAULT_PROXY_TEST_URL,
    PROXY_SETTINGS_FILE,
};
