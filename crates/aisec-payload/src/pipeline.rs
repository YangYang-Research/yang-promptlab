use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::error::PayloadResult;
use crate::library::PayloadDatabase;
use crate::mutation::{MutationEngine, MutatedVariant};
use crate::types::{
    GeneratedPayload, GenerationReport, GenerationStats, MutationKind, PayloadCategory,
    PayloadRecord,
};

/// Request to the payload generation pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub categories: Option<Vec<PayloadCategory>>,
    pub payload_ids: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub mutations: Vec<MutationKind>,
    pub max_variants_per_payload: Option<usize>,
}

/// End-to-end payload generation pipeline: library → mutation → variants.
pub struct PayloadPipeline {
    database: PayloadDatabase,
    mutator: MutationEngine,
}

impl PayloadPipeline {
    pub fn new(database: PayloadDatabase, mutator: MutationEngine) -> Self {
        Self { database, mutator }
    }

    pub fn with_defaults() -> PayloadResult<Self> {
        Ok(Self {
            database: PayloadDatabase::builtin()?,
            mutator: MutationEngine::with_defaults(),
        })
    }

    pub fn database(&self) -> &PayloadDatabase {
        &self.database
    }

    pub fn mutator(&self) -> &MutationEngine {
        &self.mutator
    }

    /// Run the full generation pipeline.
    #[instrument(skip(self, request))]
    pub fn generate(&self, request: &GenerateRequest) -> PayloadResult<GenerationReport> {
        let sources = self.select_sources(request);
        let source_count = sources.len();
        let max_per_source = request.max_variants_per_payload.unwrap_or(8);
        let mutations = if request.mutations.is_empty() {
            MutationKind::encoding_kinds().to_vec()
        } else {
            request.mutations.clone()
        };

        let mut variants = Vec::new();
        let mut mutations_applied = 0usize;

        for source in sources {
            let expanded = self.mutator.expand(&source.content, &mutations)?;
            for MutatedVariant {
                content,
                mutations: applied,
            } in expanded.into_iter().take(max_per_source)
            {
                mutations_applied += applied.len();
                variants.push(GeneratedPayload {
                    generation_id: Uuid::new_v4().to_string(),
                    source_id: source.id.clone(),
                    source_name: source.name.clone(),
                    category: source.category,
                    content,
                    mutations: applied,
                });
            }
        }

        debug!(
            sources = source_count,
            variants = variants.len(),
            "payload generation complete"
        );

        Ok(GenerationReport {
            stats: GenerationStats {
                source_count,
                variant_count: variants.len(),
                mutations_applied,
            },
            variants,
        })
    }

    /// Generate encoding variants for arbitrary content (no library lookup).
    pub fn generate_from_content(
        &self,
        content: &str,
        category: PayloadCategory,
        mutations: &[MutationKind],
    ) -> PayloadResult<Vec<GeneratedPayload>> {
        let synthetic = PayloadRecord {
            id: "inline".into(),
            name: "inline".into(),
            category,
            content: content.into(),
            tags: vec![],
            description: None,
        };

        let expanded = self.mutator.expand(content, mutations)?;
        Ok(expanded
            .into_iter()
            .map(|v| GeneratedPayload {
                generation_id: Uuid::new_v4().to_string(),
                source_id: synthetic.id.clone(),
                source_name: synthetic.name.clone(),
                category,
                content: v.content,
                mutations: v.mutations,
            })
            .collect())
    }

    fn select_sources(&self, request: &GenerateRequest) -> Vec<&PayloadRecord> {
        if let Some(ids) = &request.payload_ids {
            return ids
                .iter()
                .filter_map(|id| self.database.get(id))
                .collect();
        }

        let mut sources: Vec<&PayloadRecord> = if let Some(categories) = &request.categories {
            categories
                .iter()
                .flat_map(|c| self.database.by_category(*c))
                .collect()
        } else {
            self.database.all().iter().collect()
        };

        if let Some(tags) = &request.tags {
            let tag_set: std::collections::HashSet<_> = tags.iter().map(|t| t.to_lowercase()).collect();
            sources.retain(|r| {
                r.tags
                    .iter()
                    .any(|t| tag_set.contains(&t.to_lowercase()))
            });
        }

        sources.sort_by_key(|r| r.id.as_str());
        sources.dedup_by_key(|r| r.id.as_str());
        sources
    }
}

impl Default for PayloadPipeline {
    fn default() -> Self {
        Self::with_defaults().expect("default pipeline")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_generates_encoding_variants() {
        let pipeline = PayloadPipeline::with_defaults().unwrap();
        let report = pipeline
            .generate(&GenerateRequest {
                payload_ids: Some(vec!["enc-ignore-rules".into()]),
                mutations: MutationKind::encoding_kinds().to_vec(),
                ..Default::default()
            })
            .unwrap();

        assert!(report.stats.variant_count >= 4);
        assert!(report.variants.iter().any(|v| v.mutations.contains(&MutationKind::Base64Encode)));
        assert!(report.variants.iter().any(|v| v.mutations.contains(&MutationKind::HtmlEncode)));
    }

    #[test]
    fn pipeline_filters_by_category() {
        let pipeline = PayloadPipeline::with_defaults().unwrap();
        let report = pipeline
            .generate(&GenerateRequest {
                categories: Some(vec![PayloadCategory::Jailbreak]),
                mutations: vec![MutationKind::HexEncode],
                ..Default::default()
            })
            .unwrap();

        assert!(report.stats.source_count >= 3);
        assert!(report.variants.iter().all(|v| v.category == PayloadCategory::Jailbreak));
    }
}
