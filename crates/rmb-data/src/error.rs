//! Error type for the data layer. Repositories return [`DataError`]; the Tauri layer maps
//! it to its own `AppError` for the frontend.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DataError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("{0}")]
    Other(String),
}
