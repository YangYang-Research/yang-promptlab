use std::sync::{Arc, RwLock};

use crate::error::{HarnessError, HarnessResult};
use crate::models::{HarnessKind, TargetDescriptor};
use crate::providers::{HttpHarness, OpenAiHarness};
#[cfg(feature = "playwright")]
use crate::providers::PlaywrightHarness;
use crate::registry::HarnessRegistry;
use crate::traits::{DefaultResponseNormalizer, Harness, ResponseNormalizer};

/// Resolves harness implementations via a shared [`HarnessRegistry`].
#[derive(Clone)]
pub struct HarnessFactory {
    registry: Arc<RwLock<HarnessRegistry>>,
    normalizer: DefaultResponseNormalizer,
}

impl HarnessFactory {
    pub fn new() -> HarnessResult<Self> {
        let mut registry = HarnessRegistry::new();
        registry.register(Arc::new(HttpHarness::new()?));
        registry.register(Arc::new(OpenAiHarness::new()?));
        Ok(Self {
            registry: Arc::new(RwLock::new(registry)),
            normalizer: DefaultResponseNormalizer,
        })
    }

    /// Build a factory from a pre-populated registry (tests and custom runtimes).
    pub fn from_registry(registry: HarnessRegistry) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry)),
            normalizer: DefaultResponseNormalizer,
        }
    }

    pub fn registry(&self) -> Arc<RwLock<HarnessRegistry>> {
        self.registry.clone()
    }

    pub fn register(&self, harness: Arc<dyn Harness>) -> HarnessResult<()> {
        self.registry
            .write()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .register(harness);
        Ok(())
    }

    pub fn registered_ids(&self) -> HarnessResult<Vec<String>> {
        Ok(self
            .registry
            .read()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .ids())
    }

    #[cfg(feature = "playwright")]
    pub fn with_playwright(self, harness: PlaywrightHarness) -> Self {
        if let Ok(mut registry) = self.registry.write() {
            registry.register(Arc::new(harness));
        }
        self
    }

    pub fn resolve(&self, descriptor: &TargetDescriptor) -> HarnessResult<Arc<dyn Harness>> {
        self.resolve_kind(descriptor.preferred_harness())
    }

    pub fn resolve_by_id(&self, harness_id: &str) -> HarnessResult<Arc<dyn Harness>> {
        self.registry
            .read()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .get(harness_id)
    }

    pub fn resolve_kind(&self, kind: HarnessKind) -> HarnessResult<Arc<dyn Harness>> {
        self.registry
            .read()
            .map_err(|_| HarnessError::config("harness registry lock poisoned"))?
            .get_kind(kind)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TargetSurface;

    #[test]
    fn factory_delegates_to_registry() {
        let factory = HarnessFactory::new().unwrap();
        let ids = factory.registered_ids().unwrap();
        assert!(ids.contains(&"http".to_string()));
        assert!(ids.contains(&"openai".to_string()));

        let descriptor = TargetDescriptor {
            url: "https://api.example.com/v1/chat".into(),
            surface: TargetSurface::OpenAiCompatible,
            ..TargetDescriptor::default()
        };
        let harness = factory.resolve(&descriptor).unwrap();
        assert_eq!(harness.id(), "openai");
    }
}
