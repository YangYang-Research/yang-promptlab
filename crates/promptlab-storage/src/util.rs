use time::OffsetDateTime;
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

pub fn ensure_rows_affected(
    result: sqlx::sqlite::SqliteQueryResult,
    entity: &str,
) -> promptlab_core::PromptLabResult<()> {
    if result.rows_affected() == 0 {
        return Err(promptlab_core::PromptLabError::not_found(format!("{entity} not found")));
    }
    Ok(())
}
