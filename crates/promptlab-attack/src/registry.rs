use std::collections::HashMap;
use std::sync::Arc;

use crate::category::AttackCategory;
use crate::error::{AttackError, AttackResult};
use crate::traits::Attack;

/// Registry of attack implementations keyed by id and category.
pub struct AttackRegistry {
    by_id: HashMap<String, Arc<dyn Attack>>,
    by_category: HashMap<AttackCategory, Arc<dyn Attack>>,
}

impl AttackRegistry {
    pub fn new() -> Self {
        Self {
            by_id: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_all(crate::attacks::builtin_attacks());
        registry
    }

    pub fn register(&mut self, attack: Arc<dyn Attack>) {
        self.by_category.insert(attack.category(), attack.clone());
        self.by_id.insert(attack.id().to_string(), attack);
    }

    pub fn register_all(&mut self, attacks: Vec<Arc<dyn Attack>>) {
        for attack in attacks {
            self.register(attack);
        }
    }

    pub fn get(&self, id: &str) -> AttackResult<Arc<dyn Attack>> {
        self.by_id
            .get(id)
            .cloned()
            .ok_or_else(|| AttackError::NotFound(id.to_string()))
    }

    pub fn get_by_category(&self, category: AttackCategory) -> AttackResult<Arc<dyn Attack>> {
        self.by_category
            .get(&category)
            .cloned()
            .ok_or_else(|| AttackError::NotFound(category.as_str().to_string()))
    }

    pub fn list(&self) -> Vec<Arc<dyn Attack>> {
        self.by_id.values().cloned().collect()
    }

    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<_> = self.by_id.keys().cloned().collect();
        ids.sort();
        ids
    }
}

impl Default for AttackRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_register_all_categories() {
        let registry = AttackRegistry::with_builtins();
        assert_eq!(registry.list().len(), 9);
        for cat in AttackCategory::all() {
            assert!(registry.get_by_category(*cat).is_ok());
        }
    }
}
