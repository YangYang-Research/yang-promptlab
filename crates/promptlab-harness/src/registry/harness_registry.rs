use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{HarnessError, HarnessResult};
use crate::models::HarnessKind;
use crate::traits::Harness;

/// Registry of named harness implementations.
pub struct HarnessRegistry {
    entries: HashMap<String, Arc<dyn Harness>>,
}

impl HarnessRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn register(&mut self, harness: Arc<dyn Harness>) {
        self.entries.insert(harness.id().to_string(), harness);
    }

    pub fn get(&self, id: &str) -> HarnessResult<Arc<dyn Harness>> {
        self.entries
            .get(id)
            .cloned()
            .ok_or_else(|| HarnessError::NotFound(id.to_string()))
    }

    pub fn get_kind(&self, kind: HarnessKind) -> HarnessResult<Arc<dyn Harness>> {
        self.get(kind.as_str())
    }

    pub fn ids(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }
}

impl Default for HarnessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::HttpHarness;

    #[test]
    fn registers_and_resolves_harness() {
        let mut registry = HarnessRegistry::new();
        registry.register(Arc::new(HttpHarness::default()));
        let harness = registry.get("http").unwrap();
        assert_eq!(harness.id(), "http");
    }
}
