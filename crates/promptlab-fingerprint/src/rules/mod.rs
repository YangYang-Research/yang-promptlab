use crate::types::AiProvider;

/// Kind of signal a rule evaluates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalKind {
    UrlHost,
    UrlPath,
    Header,
    ResponseBody,
    StatusCode,
}

/// Matcher for a fingerprint rule.
#[derive(Debug, Clone)]
pub enum RuleMatcher {
    HostContains(&'static str),
    HostRegex(&'static str),
    UrlContains(&'static str),
    PathRegex(&'static str),
    HeaderPresent(&'static str),
    HeaderContains { name: &'static str, value: &'static str },
    BodyContains(&'static str),
    BodyJsonField { pointer: &'static str, equals: Option<&'static str> },
    BodyJsonArrayContains { pointer: &'static str, field: &'static str, value: &'static str },
    StatusIn(&'static [u16]),
}

/// A weighted detection rule for a specific provider.
#[derive(Debug, Clone)]
pub struct FingerprintRule {
    pub id: &'static str,
    pub provider: AiProvider,
    pub kind: SignalKind,
    pub weight: f32,
    pub matcher: RuleMatcher,
    pub description: &'static str,
}

/// Stack detection rule for agent frameworks and AI components.
#[derive(Debug, Clone)]
pub struct StackRule {
    pub id: &'static str,
    pub target: StackTarget,
    pub kind: SignalKind,
    pub weight: f32,
    pub method: crate::types::FingerprintMethod,
    pub matcher: RuleMatcher,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackTarget {
    Framework(crate::types::AgentFramework),
    Component(crate::types::AiComponent),
}

impl StackRule {
    pub const fn framework(
        id: &'static str,
        framework: crate::types::AgentFramework,
        kind: SignalKind,
        weight: f32,
        method: crate::types::FingerprintMethod,
        matcher: RuleMatcher,
        description: &'static str,
    ) -> Self {
        Self {
            id,
            target: StackTarget::Framework(framework),
            kind,
            weight,
            method,
            matcher,
            description,
        }
    }

    pub const fn component(
        id: &'static str,
        component: crate::types::AiComponent,
        kind: SignalKind,
        weight: f32,
        method: crate::types::FingerprintMethod,
        matcher: RuleMatcher,
        description: &'static str,
    ) -> Self {
        Self {
            id,
            target: StackTarget::Component(component),
            kind,
            weight,
            method,
            matcher,
            description,
        }
    }
}

impl FingerprintRule {
    pub const fn new(
        id: &'static str,
        provider: AiProvider,
        kind: SignalKind,
        weight: f32,
        matcher: RuleMatcher,
        description: &'static str,
    ) -> Self {
        Self {
            id,
            provider,
            kind,
            weight,
            matcher,
            description,
        }
    }
}

pub mod providers;
pub mod stack;

use providers::all_rules;
use stack::{component_rules, framework_rules};

/// Returns the full rule set for all supported providers.
pub fn rule_catalog() -> Vec<FingerprintRule> {
    all_rules()
}

pub fn stack_rule_catalog() -> Vec<StackRule> {
    let mut rules = framework_rules();
    rules.extend(component_rules());
    rules
}
