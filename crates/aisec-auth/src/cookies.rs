use aisec_core::{AisecError, AisecResult};

use crate::playwright::PlaywrightDriver;
use crate::session::SessionStore;
use crate::types::{CookieRecord, ExtractedToken};

/// Manages browser cookie jars for authenticated sessions.
pub struct CookieManager<'a> {
    store: &'a SessionStore,
    driver: &'a dyn PlaywrightDriver,
}

impl<'a> CookieManager<'a> {
    pub fn new(store: &'a SessionStore, driver: &'a dyn PlaywrightDriver) -> Self {
        Self { store, driver }
    }

    pub async fn export_cookies(&self, session_id: &str) -> AisecResult<Vec<CookieRecord>> {
        let session = self.store.get_session(session_id).await?;
        Ok(session.cookies)
    }

    pub async fn import_cookies(
        &self,
        session_id: &str,
        cookies: Vec<CookieRecord>,
    ) -> AisecResult<Vec<CookieRecord>> {
        let updated = self.driver.set_cookies(cookies).await?;
        self.store
            .update_session_cookies(session_id, &updated)
            .await?;
        Ok(updated)
    }

    pub async fn sync_from_browser(
        &self,
        session_id: &str,
        url: Option<&str>,
    ) -> AisecResult<Vec<CookieRecord>> {
        let cookies = self.driver.get_cookies(url).await?;
        self.store
            .update_session_cookies(session_id, &cookies)
            .await?;
        Ok(cookies)
    }

    pub fn merge_cookies(existing: &[CookieRecord], incoming: &[CookieRecord]) -> Vec<CookieRecord> {
        let mut map = std::collections::HashMap::new();
        for c in existing {
            map.insert((c.name.clone(), c.domain.clone(), c.path.clone()), c.clone());
        }
        for c in incoming {
            map.insert((c.name.clone(), c.domain.clone(), c.path.clone()), c.clone());
        }
        map.into_values().collect()
    }
}

/// Extracts and validates tokens from sessions and JWT configs.
pub struct TokenExtractor;

impl TokenExtractor {
    pub fn from_jwt_config(token: &str) -> ExtractedToken {
        ExtractedToken {
            kind: "jwt".into(),
            source: "config".into(),
            value: token.to_string(),
            url: None,
        }
    }

    pub fn validate_jwt_structure(token: &str) -> AisecResult<()> {
        use jsonwebtoken::decode_header;

        decode_header(token)
            .map_err(|e| AisecError::invalid_input(format!("invalid JWT header: {e}")))?;

        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() < 2 {
            return Err(AisecError::invalid_input(
                "JWT must contain at least header and payload",
            ));
        }

        Ok(())
    }

    pub fn merge_tokens(
        existing: &[ExtractedToken],
        incoming: &[ExtractedToken],
    ) -> Vec<ExtractedToken> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for t in existing.iter().chain(incoming.iter()) {
            let key = format!("{}:{}", t.kind, t.value);
            if seen.insert(key) {
                out.push(t.clone());
            }
        }
        out
    }

    pub fn bearer_header(token: &ExtractedToken, prefix: &str) -> String {
        if token.value.to_lowercase().starts_with("bearer ") {
            token.value.clone()
        } else {
            format!("{prefix} {}", token.value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_cookie_jars() {
        let a = vec![CookieRecord {
            name: "sid".into(),
            value: "1".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expires: None,
            http_only: true,
            secure: true,
            same_site: None,
        }];
        let b = vec![CookieRecord {
            name: "sid".into(),
            value: "2".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expires: None,
            http_only: true,
            secure: true,
            same_site: None,
        }];
        let merged = CookieManager::merge_cookies(&a, &b);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].value, "2");
    }
}
