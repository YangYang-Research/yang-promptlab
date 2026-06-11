use std::future::Future;

use aisec_core::AisecResult;
use tracing::{debug, warn};

use crate::config::RetryConfig;

/// Returns true when an HTTP status is worth retrying.
pub fn is_retryable_status(status: u16) -> bool {
    status == 408 || status == 429 || (500..=599).contains(&status)
}

/// Returns true when a reqwest error is transient.
pub fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

/// Execute `operation` with exponential backoff per `config`.
pub async fn with_retry<T, F, Fut>(
    label: &str,
    config: &RetryConfig,
    mut operation: F,
) -> AisecResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = AisecResult<T>>,
{
    let mut attempt = 0;

    loop {
        attempt += 1;
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if attempt < config.max_attempts && is_retryable_aisec(&err) => {
                let delay = config.delay_for_attempt(attempt);
                warn!(
                    label,
                    attempt,
                    max_attempts = config.max_attempts,
                    ?delay,
                    error = %err.client_message(),
                    "retrying after transient failure"
                );
                tokio::time::sleep(delay).await;
            }
            Err(err) => return Err(err),
        }
    }
}

fn is_retryable_aisec(err: &aisec_core::AisecError) -> bool {
    matches!(
        err.client_message().as_str(),
        msg if msg.contains("timeout")
            || msg.contains("connection")
            || msg.contains("429")
            || msg.contains("503")
            || msg.contains("502")
            || msg.contains("504")
    ) || err.client_message().starts_with('5')
}

/// Retry wrapper for raw reqwest operations returning Result<T, reqwest::Error>.
pub async fn with_reqwest_retry<T, F, Fut>(
    label: &str,
    config: &RetryConfig,
    mut operation: F,
) -> Result<T, reqwest::Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, reqwest::Error>>,
{
    let mut attempt = 0;

    loop {
        attempt += 1;
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) if attempt < config.max_attempts && is_retryable_error(&err) => {
                let delay = config.delay_for_attempt(attempt);
                debug!(label, attempt, ?delay, error = %err, "reqwest retry");
                tokio::time::sleep(delay).await;
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisec_core::AisecError;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn retries_until_success() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = calls.clone();
        let cfg = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            multiplier: 2.0,
        };

        let result = with_retry("test", &cfg, || {
            let calls = calls_clone.clone();
            async move {
                let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
                if n < 3 {
                    Err(AisecError::internal("503 service unavailable"))
                } else {
                    Ok("ok")
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
}
