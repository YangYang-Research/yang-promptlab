#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptId {
    InferenceSystem,
    HealthCheckSystem,
    HealthCheckUser,
    JudgeSystem,
    JudgeUser,
    ClassifierSystem,
    ClassifierUser,
    AttackerSystem,
    AttackerUser,
    PlannerSystem,
    PlannerUser,
    GeneratorSystem,
    GeneratorUser,
    ReportExecutiveSummary,
    ReportRiskSummary,
    ReportMitigationSummary,
}

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub id: PromptId,
    pub system: Option<String>,
    pub user: String,
}

impl PromptTemplate {
    pub fn render_user(&self, vars: &[(&str, &str)]) -> String {
        let mut out = self.user.clone();
        for (key, value) in vars {
            out = out.replace(&format!("{{{key}}}"), value);
        }
        out
    }
}
