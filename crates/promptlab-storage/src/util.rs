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
) -> aisec_core::AisecResult<()> {
    if result.rows_affected() == 0 {
        return Err(aisec_core::AisecError::not_found(format!("{entity} not found")));
    }
    Ok(())
}
