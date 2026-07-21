use async_trait::async_trait;
use sqlx::SqlitePool;

use aisec_core::{AisecError, AisecResult};

use crate::error::StorageResultExt;
use crate::models::{JudgeRoleWeights, UpdateJudgeRoleWeights};
use crate::repositories::JudgeRoleWeightsRepository;
use crate::util::now;

#[derive(Clone)]
pub struct SqliteJudgeRoleWeightsRepository {
    pool: SqlitePool,
}

impl SqliteJudgeRoleWeightsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn validate_weight(name: &str, value: f64) -> AisecResult<()> {
    if !(0.01..=2.0).contains(&value) || !value.is_finite() {
        return Err(AisecError::invalid_input(format!(
            "{name} weight must be between 0.01 and 2.0"
        )));
    }
    Ok(())
}

#[async_trait]
impl JudgeRoleWeightsRepository for SqliteJudgeRoleWeightsRepository {
    async fn get(&self) -> AisecResult<JudgeRoleWeights> {
        let row = sqlx::query_as::<_, JudgeRoleWeights>(
            r#"
            SELECT id, judge, classifier, attacker, default_llm, updated_at
            FROM judge_role_weights
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_storage()?;

        if let Some(row) = row {
            return Ok(row);
        }

        // Migration seed missing — insert defaults.
        let timestamp = now();
        sqlx::query(
            r#"
            INSERT INTO judge_role_weights (id, judge, classifier, attacker, default_llm, updated_at)
            VALUES (1, 0.85, 0.75, 0.70, 0.65, ?)
            "#,
        )
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        Ok(JudgeRoleWeights {
            id: 1,
            judge: 0.85,
            classifier: 0.75,
            attacker: 0.70,
            default_llm: 0.65,
            updated_at: timestamp,
        })
    }

    async fn update(&self, input: UpdateJudgeRoleWeights) -> AisecResult<JudgeRoleWeights> {
        validate_weight("judge", input.judge)?;
        validate_weight("classifier", input.classifier)?;
        validate_weight("attacker", input.attacker)?;
        validate_weight("default_llm", input.default_llm)?;

        let timestamp = now();
        sqlx::query(
            r#"
            UPDATE judge_role_weights
            SET judge = ?, classifier = ?, attacker = ?, default_llm = ?, updated_at = ?
            WHERE id = 1
            "#,
        )
        .bind(input.judge)
        .bind(input.classifier)
        .bind(input.attacker)
        .bind(input.default_llm)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_storage()?;

        self.get().await
    }
}
