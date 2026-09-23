//! Tauri command handlers — thin adapters that deserialize args, call `rmb-domain` /
//! `rmb-data` logic, and map errors to [`AppError`]. Keep bodies minimal: real logic lives
//! in the domain/data crates so it stays unit-testable without a webview.

pub mod backup;
pub mod catalog;
pub mod customers;
pub mod invoicing;
pub mod jobs;
pub mod pdf;
pub mod quoting;
pub mod recurring;
pub mod reports;
pub mod settings;

use rmb_data::db::Db;
use rmb_data::repos::meta;
use tauri::State;

use crate::error::AppError;

/// Walking-skeleton command: greet via the pure domain function.
#[tauri::command]
pub fn greet(name: &str) -> Result<String, AppError> {
    Ok(rmb_domain::greet(name))
}

/// App-meta keys the interface may read. Everything else in `app_meta` (the schema marker,
/// upgrade notices) belongs to the backend.
const READABLE_META: [&str; 2] = ["ui.theme", "startup.warning"];

/// Read an app-meta value (e.g. a UI preference) by key.
#[tauri::command]
pub async fn get_meta(db: State<'_, Db>, key: String) -> Result<Option<String>, AppError> {
    if !READABLE_META.contains(&key.as_str()) {
        return Err(AppError::Message(format!("unknown setting '{key}'")));
    }
    Ok(meta::get(&db, &key).await?)
}

/// Save a UI preference. Only the theme is writable from the interface.
#[tauri::command]
pub async fn set_meta(db: State<'_, Db>, key: String, value: String) -> Result<(), AppError> {
    if key != "ui.theme" || !matches!(value.as_str(), "light" | "dark") {
        return Err(AppError::Message(format!(
            "'{key}' can't be set to '{value}'"
        )));
    }
    meta::set(&db, &key, &value).await?;
    Ok(())
}

/// Clear the dashboard's startup warning once the user has read it.
#[tauri::command]
pub async fn dismiss_startup_warning(db: State<'_, Db>) -> Result<(), AppError> {
    meta::set(&db, "startup.warning", "").await?;
    Ok(())
}
