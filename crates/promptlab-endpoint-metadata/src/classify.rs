use crate::capability::endpoint_type_hints;
use crate::types::{
    EndpointCapabilities, EndpointClassification, EndpointType, FingerprintMetadata,
};

pub struct EndpointClassifier;

impl EndpointClassifier {
    pub fn classify(
        url: &str,
        kind: &str,
        fingerprint: &FingerprintMetadata,
        capabilities: &EndpointCapabilities,
        discovery_confidence: f64,
    ) -> EndpointClassification {
        let endpoint_type = endpoint_type_hints(capabilities, url, kind);
        let confidence = classify_confidence(endpoint_type, fingerprint.confidence, discovery_confidence);

        EndpointClassification {
            endpoint_type,
            ai_framework: if fingerprint.framework.is_empty() {
                fingerprint.provider.clone()
            } else {
                fingerprint.framework.clone()
            },
            confidence,
            risk_score: 0, // filled by RiskScorer
        }
    }
}

fn classify_confidence(
    endpoint_type: EndpointType,
    fingerprint_confidence: f32,
    discovery_confidence: f64,
) -> f32 {
    let base = ((fingerprint_confidence + discovery_confidence as f32) / 2.0).clamp(0.0, 1.0);
    if endpoint_type == EndpointType::NonAi {
        (base * 0.5).clamp(0.0, 1.0)
    } else {
        base
    }
}
