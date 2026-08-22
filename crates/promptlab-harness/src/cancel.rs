use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::{HarnessError, HarnessResult};

/// Cooperative cancellation flag shared with in-flight harness I/O.
#[derive(Clone, Debug, Default)]
pub struct CancelFlag {
    inner: Arc<AtomicBool>,
}

impl CancelFlag {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn from_shared(inner: Arc<AtomicBool>) -> Self {
        Self { inner }
    }

    pub fn shared(&self) -> Arc<AtomicBool> {
        self.inner.clone()
    }

    pub fn reset(&self) {
        self.inner.store(false, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> HarnessResult<()> {
        if self.is_cancelled() {
            Err(HarnessError::Cancelled)
        } else {
            Ok(())
        }
    }
}
