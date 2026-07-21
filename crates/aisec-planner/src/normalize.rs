use aisec_attack::AttackCategory;

pub fn parse_attack_category(raw: &str) -> Option<AttackCategory> {
    AttackCategory::all()
        .iter()
        .copied()
        .find(|cat| cat.as_str() == raw)
}
