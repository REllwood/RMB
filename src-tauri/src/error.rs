//! Application-level error type returned by Tauri commands.
//!
//! Commands return `Result<T, AppError>`; `AppError` serializes to a plain string so the
//! React frontend receives a readable message. Domain/data error variants are added as
//! those layers come online (Phase B onward).

use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        AppError::Message(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        AppError::Message(value.to_string())
    }
}

// Tauri requires command error types to be `Serialize`. Emit the human-readable message.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
