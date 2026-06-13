//! Curated model catalog — backed by `resources/models.json` at runtime.

use crate::builtin_catalog::BuiltinCatalog;
use crate::types::ModelCatalogEntry;

/// Lookup a catalog entry by id from the provided catalog slice.
pub fn find_catalog_entry<'a>(
    catalog: &'a [ModelCatalogEntry],
    catalog_id: &str,
) -> Option<&'a ModelCatalogEntry> {
    catalog.iter().find(|entry| entry.id == catalog_id)
}

/// Deprecated hardcoded catalog — returns empty; use [`BuiltinCatalog`] instead.
#[deprecated(note = "use BuiltinCatalog loaded from resources/models.json")]
pub fn curated_catalog() -> Vec<ModelCatalogEntry> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_catalog::{BuiltinRegistryEntry, entry_to_catalog};

    #[test]
    fn find_in_catalog_slice() {
        let entry = entry_to_catalog(&BuiltinRegistryEntry {
            id: "test".into(),
            name: "Test".into(),
            purpose: "general".into(),
            recommended: false,
            provider: "ollama".into(),
            repo: None,
            file: None,
            ollama_tag: Some("llama3".into()),
            sha256: None,
        });
        let catalog = vec![entry];
        assert!(find_catalog_entry(&catalog, "test").is_some());
    }
}
