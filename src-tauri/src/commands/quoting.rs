//! Quote commands.

use rmb_data::db::Db;
use rmb_data::repos::invoices::LineInput;
use rmb_data::repos::quotes::{self, QuoteDetail, QuoteRow};
use tauri::State;

use crate::error::AppError;

#[tauri::command]
pub async fn list_quotes(db: State<'_, Db>) -> Result<Vec<QuoteRow>, AppError> {
    Ok(quotes::list(&db).await?)
}

#[tauri::command]
pub async fn get_quote(db: State<'_, Db>, id: i64) -> Result<Option<QuoteDetail>, AppError> {
    Ok(quotes::get_detail(&db, id).await?)
}

#[tauri::command]
pub async fn create_quote(
    db: State<'_, Db>,
    customer_id: i64,
    lines: Vec<LineInput>,
    valid_until: Option<String>,
    notes: String,
) -> Result<i64, AppError> {
    Ok(quotes::create_draft(&db, customer_id, &lines, valid_until.as_deref(), &notes).await?)
}

#[tauri::command]
pub async fn set_quote_status(db: State<'_, Db>, id: i64, status: String) -> Result<(), AppError> {
    quotes::set_status(&db, id, &status).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_quote(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    quotes::delete(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn convert_quote_to_invoice(db: State<'_, Db>, id: i64) -> Result<i64, AppError> {
    Ok(quotes::convert_to_invoice(&db, id).await?)
}
