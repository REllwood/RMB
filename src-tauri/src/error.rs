//! Application-level error type returned by Tauri commands.
//!
//! Commands return `Result<T, AppError>`; `AppError` serializes to a plain string so the
//! React frontend receives a readable message. Data-layer errors convert in automatically.

use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    Data(String),
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

impl From<rmb_data::DataError> for AppError {
    fn from(value: rmb_data::DataError) -> Self {
        AppError::Data(value.user_message())
    }
}

// Tauri requires command error types to be `Serialize`. Emit the human-readable message.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
