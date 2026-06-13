//! Persistent browser authentication session framework.

pub mod error;
pub mod paths;
pub mod session;

pub use error::{BrowserError, BrowserResult};
pub use paths::{auth_sessions_dir, default_data_root};
pub use session::{BrowserSessionManager, BrowserSessionRecord, BrowserSessionStatus};
