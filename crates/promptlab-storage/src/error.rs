use aisec_core::{AisecError, ErrorCode};

pub fn map_sqlx_error(err: sqlx::Error) -> AisecError {
    match err {
        sqlx::Error::RowNotFound => AisecError::not_found("record not found"),
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            let message = format!("unique constraint violation: {db_err}");
            AisecError::Tagged {
                code: ErrorCode::Storage,
                message,
                source: None,
            }
        }
        sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
            AisecError::invalid_input(format!("foreign key violation: {db_err}"))
        }
        other => AisecError::Tagged {
            code: ErrorCode::Storage,
            message: other.to_string(),
            source: Some(Box::new(other)),
        },
    }
}

pub trait StorageResultExt<T> {
    fn map_storage(self) -> aisec_core::AisecResult<T>;
}

impl<T> StorageResultExt<T> for Result<T, sqlx::Error> {
    fn map_storage(self) -> aisec_core::AisecResult<T> {
        self.map_err(map_sqlx_error)
    }
}
