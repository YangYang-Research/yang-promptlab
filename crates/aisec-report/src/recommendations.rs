use uuid::Uuid;

use crate::types::{Recommendation, ReportFinding, Severity};

/// Generate remediation recommendations from findings.
pub fn generate_recommendations(findings: &[ReportFinding]) -> Vec<Recommendation> {
    let mut recs = Vec::new();

    let has_critical = findings.iter().any(|f| f.severity == Severity::Critical);
    let has_injection = findings
        .iter()
        .any(|f| f.category.contains("injection") || f.category.contains("prompt"));
    let has_jailbreak = findings.iter().any(|f| f.category.contains("jailbreak"));
    let has_rag = findings.iter().any(|f| f.category.contains("rag"));
    let has_tool = findings
        .iter()
        .any(|f| f.category.contains("tool") || f.category.contains("mcp"));

    if has_critical || has_injection {
        recs.push(Recommendation {
            id: Uuid::new_v4().to_string(),
            priority: Severity::Critical,
            title: "Implement input/output guardrails".into(),
            description: "Deploy prompt injection filters, system prompt isolation, and output validation before responses reach users.".into(),
            related_findings: related_ids(findings, &["injection", "prompt"]),
        });
    }

    if has_jailbreak {
        recs.push(Recommendation {
            id: Uuid::new_v4().to_string(),
            priority: Severity::High,
            title: "Strengthen safety classifiers".into(),
            description: "Add multi-layer jailbreak detection, refusal reinforcement, and adversarial fine-tuning on known bypass patterns.".into(),
            related_findings: related_ids(findings, &["jailbreak"]),
        });
    }

    if has_rag {
        recs.push(Recommendation {
            id: Uuid::new_v4().to_string(),
            priority: Severity::High,
            title: "Harden RAG retrieval boundaries".into(),
            description: "Sanitize retrieved context, enforce source attribution controls, and prevent raw document exfiltration in responses.".into(),
            related_findings: related_ids(findings, &["rag"]),
        });
    }

    if has_tool {
        recs.push(Recommendation {
            id: Uuid::new_v4().to_string(),
            priority: Severity::Critical,
            title: "Restrict tool and MCP permissions".into(),
            description: "Apply least-privilege tool schemas, parameter validation, and human-in-the-loop approval for sensitive actions.".into(),
            related_findings: related_ids(findings, &["tool", "mcp"]),
        });
    }

    if findings.is_empty() {
        recs.push(Recommendation {
            id: Uuid::new_v4().to_string(),
            priority: Severity::Info,
            title: "Maintain continuous testing".into(),
            description: "No findings in this scan. Continue periodic AI red-team assessments as models and prompts evolve.".into(),
            related_findings: vec![],
        });
    } else {
        recs.push(Recommendation {
            id: Uuid::new_v4().to_string(),
            priority: Severity::Medium,
            title: "Establish AI security monitoring".into(),
            description: "Log probe anomalies, track guardrail trigger rates, and integrate findings into your vulnerability management workflow.".into(),
            related_findings: findings.iter().map(|f| f.id.clone()).collect(),
        });
    }

    recs
}

pub fn recommendation_for(category: &str, severity: Severity) -> Recommendation {
    let cat = category.to_lowercase();
    let (title, description) = if cat.contains("injection") || cat.contains("prompt") {
        (
            "Mitigate prompt injection",
            "Isolate system instructions, validate user input, and apply output filtering.",
        )
    } else if cat.contains("jailbreak") {
        (
            "Address jailbreak vulnerability",
            "Update safety policies and add classifier layers for roleplay bypass attempts.",
        )
    } else if cat.contains("rag") {
        (
            "Fix RAG leakage",
            "Restrict context exposure and validate retrieval scope per user session.",
        )
    } else if cat.contains("tool") || cat.contains("mcp") {
        (
            "Lock down tool access",
            "Validate tool parameters and enforce authorization on agent actions.",
        )
    } else {
        (
            "Remediate AI security finding",
            "Review finding evidence and apply appropriate guardrails for this attack category.",
        )
    };

    Recommendation {
        id: Uuid::new_v4().to_string(),
        priority: severity,
        title: title.into(),
        description: description.into(),
        related_findings: vec![],
    }
}

pub fn compliance_refs_for(category: &str) -> Vec<String> {
    let cat = category.to_lowercase();
    let mut refs = vec!["OWASP LLM Top 10".to_string()];

    if cat.contains("injection") || cat.contains("prompt") {
        refs.push("LLM01: Prompt Injection".into());
        refs.push("NIST AI RMF: MAP 1.5".into());
    } else if cat.contains("jailbreak") {
        refs.push("LLM02: Insecure Output Handling".into());
    } else if cat.contains("rag") {
        refs.push("LLM06: Sensitive Information Disclosure".into());
    } else if cat.contains("tool") || cat.contains("mcp") || cat.contains("agent") {
        refs.push("LLM08: Excessive Agency".into());
    } else {
        refs.push("LLM09: Overreliance".into());
    }

    refs
}

fn related_ids(findings: &[ReportFinding], keywords: &[&str]) -> Vec<String> {
    findings
        .iter()
        .filter(|f| {
            let cat = f.category.to_lowercase();
            keywords.iter().any(|k| cat.contains(k))
        })
        .map(|f| f.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ReportFinding;

    #[test]
    fn generates_injection_recommendation() {
        let findings = vec![ReportFinding {
            id: "f1".into(),
            title: "PI".into(),
            severity: Severity::Critical,
            category: "prompt_injection".into(),
            description: String::new(),
            payload: None,
            response: None,
            confidence: None,
            evidence: None,
            recommendation: None,
            compliance_refs: vec![],
            status: "open".into(),
        }];
        let recs = generate_recommendations(&findings);
        assert!(recs.iter().any(|r| r.title.contains("guardrails")));
    }
}
