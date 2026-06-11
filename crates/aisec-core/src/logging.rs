use std::path::PathBuf;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::{AisecError, AisecResult};

/// Holds the non-blocking file writer guard for the lifetime of the process.
pub struct LogGuard {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

/// Logging bootstrap configuration.
#[derive(Debug, Clone)]
pub struct LogOptions {
    pub app_name: String,
    pub log_dir: Option<PathBuf>,
    pub default_filter: String,
    pub json_file: bool,
}

impl LogOptions {
    pub fn bootstrap(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            log_dir: None,
            default_filter: "info,aisec_core=debug,aisec_desktop=debug".into(),
            json_file: false,
        }
    }

    pub fn with_log_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.log_dir = Some(dir.into());
        self
    }
}

/// Initialize global tracing subscriber. Returns a guard that must be kept alive
/// when file logging is enabled.
pub fn init_logging(options: LogOptions) -> AisecResult<LogGuard> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(options.default_filter.clone()));

    let mut file_guard = None;

    if tracing::dispatcher::has_been_set() {
        return Ok(LogGuard {
            _file_guard: file_guard,
        });
    }

    let registry = tracing_subscriber::registry().with(env_filter);

    if let Some(log_dir) = options.log_dir.as_deref() {
        std::fs::create_dir_all(log_dir).map_err(AisecError::from)?;

        let file_appender = tracing_appender::rolling::daily(log_dir, "aisec.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        file_guard = Some(guard);

        let stdout_layer = fmt::layer().with_target(true).with_ansi(true);
        let file_layer = fmt::layer()
            .with_target(true)
            .with_ansi(false)
            .with_writer(non_blocking);

        let _ = options.json_file;

        registry
            .with(stdout_layer)
            .with(file_layer)
            .try_init()
            .map_err(|err| AisecError::internal(format!("failed to init file logging: {err}")))?;
    } else {
        registry
            .with(fmt::layer().with_target(true).with_ansi(true))
            .try_init()
            .map_err(|err| AisecError::internal(format!("failed to init stdout logging: {err}")))?;
    }

    tracing::info!(
        app = %options.app_name,
        log_dir = ?options.log_dir,
        "logging initialized"
    );

    Ok(LogGuard {
        _file_guard: file_guard,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_options_builder() {
        let options = LogOptions::bootstrap("aisec-test").with_log_dir("/tmp/aisec-test-logs");
        assert_eq!(options.app_name, "aisec-test");
        assert!(options.log_dir.is_some());
    }
}
