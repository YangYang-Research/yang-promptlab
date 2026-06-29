//! PromptLab core library — shared error handling, environment, OCSF logging, and domain primitives.

pub mod environment;
pub mod error;
pub mod event_log;
pub mod logging;

pub use environment::{
    bootstrap_environment, ensure_environment, load_environment_config, resolve_db_path,
    resolve_paths, save_environment_config, user_home, EnvironmentConfig, EnvironmentPaths,
    DB_FILENAME, DB_PATH_ENV, ENV_CONFIG_FILE, ROOT_DIR_NAME, ROOT_PATH_ENV,
};
pub use error::{AisecError, AisecResult, ErrorCode};
pub use event_log::{
    global_event_bus, global_event_ring, list_log_files, mask_secrets, publish_crash,
    read_log_tail, spawn_event_logger, EventBus, EventLogGuard, EventRing, LogCategory,
    OcsfEvent, OcsfSeverity,
};
pub use logging::{init_logging, LogGuard, LogOptions};
