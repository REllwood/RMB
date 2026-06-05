//! Settings + tax-rate commands.

use rmb_data::db::Db;
use rmb_data::repos::settings::{self, Settings, TaxRateRow};
use tauri::State;

use crate::error::AppError;

#[tauri::command]
pub async fn get_settings(db: State<'_, Db>) -> Result<Settings, AppError> {
    Ok(settings::get(&db).await?)
}

#[tauri::command]
pub async fn update_settings(db: State<'_, Db>, value: Settings) -> Result<(), AppError> {
    settings::update(&db, &value).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_tax_rates(db: State<'_, Db>) -> Result<Vec<TaxRateRow>, AppError> {
    Ok(settings::list_tax_rates(&db, false).await?)
}

#[tauri::command]
pub async fn create_tax_rate(
    db: State<'_, Db>,
    name: String,
    rate_bp: i64,
    inclusive: bool,
) -> Result<i64, AppError> {
    Ok(settings::create_tax_rate(&db, &name, rate_bp, inclusive).await?)
}

#[tauri::command]
pub async fn update_tax_rate(
    db: State<'_, Db>,
    id: i64,
    name: String,
    rate_bp: i64,
    inclusive: bool,
) -> Result<(), AppError> {
    settings::update_tax_rate(&db, id, &name, rate_bp, inclusive).await?;
    Ok(())
}

#[tauri::command]
pub async fn archive_tax_rate(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    settings::archive_tax_rate(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn apply_tax_preset(db: State<'_, Db>, country: String) -> Result<(), AppError> {
    settings::apply_tax_preset(&db, &country).await?;
    Ok(())
}
