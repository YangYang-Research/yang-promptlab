use std::sync::Arc;

use crate::error::{HarnessError, HarnessResult};
use crate::models::{HarnessKind, TargetDescriptor};
use crate::providers::{HttpHarness, OpenAiHarness};
#[cfg(feature = "playwright")]
use crate::providers::PlaywrightHarness;
use crate::traits::{DefaultResponseNormalizer, Harness, ResponseNormalizer};

/// Resolves the correct harness for a target descriptor.
#[derive(Clone)]
pub struct HarnessFactory {
    http: Arc<HttpHarness>,
    openai: Arc<OpenAiHarness>,
    normalizer: DefaultResponseNormalizer,
    #[cfg(feature = "playwright")]
    playwright: Option<Arc<PlaywrightHarness>>,
}

impl HarnessFactory {
    pub fn new() -> HarnessResult<Self> {
        Ok(Self {
            http: Arc::new(HttpHarness::new()?),
            openai: Arc::new(OpenAiHarness::new()?),
            normalizer: DefaultResponseNormalizer,
            #[cfg(feature = "playwright")]
            playwright: None,
        })
    }

    #[cfg(feature = "playwright")]
    pub fn with_playwright(mut self, harness: PlaywrightHarness) -> Self {
        self.playwright = Some(Arc::new(harness));
        self
    }

    pub fn resolve(&self, descriptor: &TargetDescriptor) -> HarnessResult<Arc<dyn Harness>> {
        self.resolve_kind(descriptor.preferred_harness())
    }

    pub fn resolve_kind(&self, kind: HarnessKind) -> HarnessResult<Arc<dyn Harness>> {
        match kind {
            HarnessKind::Http => Ok(self.http.clone()),
            HarnessKind::OpenAi => Ok(self.openai.clone()),
            #[cfg(feature = "playwright")]
            HarnessKind::Playwright => {
                let harness: Arc<dyn Harness> = self
                    .playwright
                    .clone()
                    .ok_or_else(|| {
                        HarnessError::NotFound(
                            "playwright harness is not configured for this runtime".into(),
                        )
                    })?;
                Ok(harness)
            }
            #[cfg(not(feature = "playwright"))]
            HarnessKind::Playwright => Err(HarnessError::NotFound(
                "playwright harness requires the `playwright` feature".into(),
            )),
        }
    }

    pub async fn execute(
        &self,
        descriptor: &TargetDescriptor,
        request: crate::models::AttackRequest,
    ) -> HarnessResult<crate::models::NormalizedResponse> {
        let harness = self.resolve(descriptor)?;
        let response = harness.execute(request.clone()).await?;
        self.normalizer.normalize(&request, response)
    }
}

impl Default for HarnessFactory {
    fn default() -> Self {
        Self::new().expect("harness factory")
    }
}
