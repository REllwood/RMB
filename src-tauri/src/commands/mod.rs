//! Tauri command handlers — thin adapters that deserialize args, call `rmb-domain` /
//! `rmb-data` logic, and map errors to [`AppError`]. Keep bodies minimal: real logic lives
//! in the domain/data crates so it stays unit-testable without a webview.

pub mod backup;
pub mod catalog;
pub mod customers;
pub mod invoicing;
pub mod jobs;
pub mod quoting;
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

/// Read an app-meta value (e.g. a UI preference) by key.
#[tauri::command]
pub async fn get_meta(db: State<'_, Db>, key: String) -> Result<Option<String>, AppError> {
    Ok(meta::get(&db, &key).await?)
}

/// Write an app-meta value (upsert).
#[tauri::command]
pub async fn set_meta(db: State<'_, Db>, key: String, value: String) -> Result<(), AppError> {
    meta::set(&db, &key, &value).await?;
    Ok(())
}
