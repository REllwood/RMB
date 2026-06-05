//! Tauri command handlers — thin adapters that deserialize args, call `rmb-domain` /
//! `rmb-data` logic, and map errors to [`AppError`]. Keep bodies minimal: real logic lives
//! in the domain/data crates so it stays unit-testable without a webview.

use crate::error::AppError;

/// Walking-skeleton command: greet via the pure domain function.
/// Proves the UI → command → domain round-trip end to end.
#[tauri::command]
pub fn greet(name: &str) -> Result<String, AppError> {
    Ok(rmb_domain::greet(name))
}
