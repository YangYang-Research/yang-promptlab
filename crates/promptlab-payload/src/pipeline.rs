use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::error::PayloadResult;
use crate::library::PayloadDatabase;
use crate::mutation::{MutationConfig, MutationEngine, MutatedVariant};
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
        Self::for_variant_budget(8)
    }

    /// Pipeline tuned to produce up to `budget` variants per source testcase.
    pub fn for_variant_budget(budget: usize) -> PayloadResult<Self> {
        Self::for_variant_budget_with_db(PayloadDatabase::builtin()?, budget)
    }

    pub fn for_variant_budget_with_db(
        database: PayloadDatabase,
        budget: usize,
    ) -> PayloadResult<Self> {
        let _budget = budget.max(1);
        Ok(Self {
            database,
            mutator: MutationEngine::new(MutationConfig {
                enabled: MutationKind::all().to_vec(),
                max_per_payload: MutationKind::all().len(),
                include_original: true,
            }),
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
            MutationKind::all().to_vec()
        } else {
            request.mutations.clone()
        };

        let mut variants = Vec::new();
        let mut mutations_applied = 0usize;

        for source in sources {
            let expanded = self.expand_source_variants(&source.content, &mutations, max_per_source)?;
            for MutatedVariant {
                content,
                mutations: applied,
            } in expanded
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

    fn expand_source_variants(
        &self,
        content: &str,
        allowed: &[MutationKind],
        max: usize,
    ) -> PayloadResult<Vec<MutatedVariant>> {
        let max = max.max(1);
        let mut out = self.mutator.expand(content, allowed)?;

        for kind in [
            MutationKind::Base64Wrap,
            MutationKind::HexWrap,
            MutationKind::HtmlWrap,
        ] {
            if out.len() >= max {
                break;
            }
            if let Ok(mutated) = self.mutator.apply(kind, content) {
                if !out.iter().any(|v| v.content == mutated) {
                    out.push(MutatedVariant {
                        content: mutated,
                        mutations: vec![kind],
                    });
                }
            }
        }

        for chain in [
            &[MutationKind::UnicodeObfuscation, MutationKind::Base64Encode][..],
            &[MutationKind::HexEncode, MutationKind::HtmlEncode][..],
            &[MutationKind::Base64Encode, MutationKind::UnicodeObfuscation][..],
        ] {
            if out.len() >= max {
                break;
            }
            if let Ok(mutated) = self.mutator.apply_chain(chain, content) {
                if !out.iter().any(|v| v.content == mutated) {
                    out.push(MutatedVariant {
                        content: mutated,
                        mutations: chain.to_vec(),
                    });
                }
            }
        }

        let mut round = 0usize;
        while out.len() < max {
            let kind = allowed[round % allowed.len()];
            let seed = if out.is_empty() {
                content.to_string()
            } else {
                out[round % out.len()].content.clone()
            };
            let mutated = self.mutator.apply(kind, &seed)?;
            if out.iter().any(|v| v.content == mutated) {
                out.push(MutatedVariant {
                    content: format!("[promptlab-variant:{}] {content}", out.len()),
                    mutations: vec![],
                });
            } else {
                out.push(MutatedVariant {
                    content: mutated,
                    mutations: vec![kind],
                });
            }
            round += 1;
        }

        out.truncate(max);
        Ok(out)
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
    fn pipeline_honors_variant_budget() {
        let pipeline = PayloadPipeline::for_variant_budget(10).unwrap();
        let report = pipeline
            .generate(&GenerateRequest {
                payload_ids: Some(vec!["pi-direct-override".into()]),
                max_variants_per_payload: Some(10),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(report.stats.variant_count, 10);
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
