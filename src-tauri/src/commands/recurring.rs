//! Recurring-invoice schedule commands. Generation also runs automatically at app startup.

use rmb_data::db::Db;
use rmb_data::repos::invoices::LineInput;
use rmb_data::repos::recurring::{self, RecurringDetail, RecurringInput, RecurringListRow};
use tauri::State;

use crate::error::AppError;

#[tauri::command]
pub async fn list_recurring(db: State<'_, Db>) -> Result<Vec<RecurringListRow>, AppError> {
    Ok(recurring::list(&db).await?)
}

#[tauri::command]
pub async fn get_recurring(
    db: State<'_, Db>,
    id: i64,
) -> Result<Option<RecurringDetail>, AppError> {
    Ok(recurring::get_detail(&db, id).await?)
}

#[tauri::command]
pub async fn create_recurring(
    db: State<'_, Db>,
    input: RecurringInput,
    lines: Vec<LineInput>,
) -> Result<i64, AppError> {
    Ok(recurring::create(&db, &input, &lines).await?)
}

#[tauri::command]
pub async fn update_recurring(
    db: State<'_, Db>,
    id: i64,
    input: RecurringInput,
    lines: Vec<LineInput>,
) -> Result<(), AppError> {
    recurring::update(&db, id, &input, &lines).await?;
    Ok(())
}

#[tauri::command]
pub async fn set_recurring_active(
    db: State<'_, Db>,
    id: i64,
    active: bool,
) -> Result<(), AppError> {
    recurring::set_active(&db, id, active).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_recurring(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    recurring::delete(&db, id).await?;
    Ok(())
}

/// Generate everything due as of today; returns how many drafts were created.
#[tauri::command]
pub async fn run_recurring_now(db: State<'_, Db>) -> Result<i64, AppError> {
    Ok(recurring::run_due_now(&db).await?.len() as i64)
}
