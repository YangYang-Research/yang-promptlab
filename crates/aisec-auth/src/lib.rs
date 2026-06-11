//! AISec Authentication Engine — Playwright-backed login recording and session management.

pub mod config;
pub mod cookies;
pub mod engine;
pub mod mock;
pub mod playwright;
pub mod session;
pub mod types;

pub use config::AuthEngineConfig;
pub use cookies::{CookieManager, TokenExtractor};
pub use engine::AuthEngine;
pub use mock::{MockPlaywrightDriver, SharedPlaywrightDriver};
pub use playwright::{PlaywrightClient, PlaywrightDriver};
pub use session::SessionStore;
pub use types::*;
