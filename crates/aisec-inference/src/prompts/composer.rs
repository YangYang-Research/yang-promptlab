use super::template::PromptTemplate;

pub struct PromptComposer;

impl PromptComposer {
    pub fn compose(system: Option<&str>, user: &str) -> String {
        match system {
            Some(sys) if !sys.trim().is_empty() => format!("{sys}\n\n{user}"),
            _ => user.to_string(),
        }
    }

    pub fn from_template(template: &PromptTemplate) -> (Option<&str>, String) {
        (template.system.as_deref(), template.user.clone())
    }
}
