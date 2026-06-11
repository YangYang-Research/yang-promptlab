//! AISec core library — shared error handling, logging, and domain primitives.

pub mod error;
pub mod logging;

pub use error::{AisecError, AisecResult, ErrorCode};
pub use logging::{init_logging, LogGuard, LogOptions};
