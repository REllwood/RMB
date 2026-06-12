//! Invoice + payment commands.

use rmb_data::db::Db;
use rmb_data::repos::dashboard::{self, DashboardSummary};
use rmb_data::repos::invoices::{self, InvoiceDetail, InvoiceRow, LineInput};
use rmb_data::repos::payments::{self, Payment};
use tauri::State;

use crate::error::AppError;

#[tauri::command]
pub async fn dashboard_summary(db: State<'_, Db>) -> Result<DashboardSummary, AppError> {
    Ok(dashboard::summary(&db).await?)
}

#[tauri::command]
pub async fn list_invoices(db: State<'_, Db>) -> Result<Vec<InvoiceRow>, AppError> {
    Ok(invoices::list(&db).await?)
}

#[tauri::command]
pub async fn get_invoice(db: State<'_, Db>, id: i64) -> Result<Option<InvoiceDetail>, AppError> {
    Ok(invoices::get_detail(&db, id).await?)
}

#[tauri::command]
pub async fn create_invoice(
    db: State<'_, Db>,
    customer_id: i64,
    lines: Vec<LineInput>,
    due_date: Option<String>,
    notes: String,
) -> Result<i64, AppError> {
    Ok(invoices::create_draft(&db, customer_id, &lines, due_date.as_deref(), &notes).await?)
}

#[tauri::command]
pub async fn update_invoice_draft(
    db: State<'_, Db>,
    id: i64,
    customer_id: i64,
    lines: Vec<LineInput>,
    due_date: Option<String>,
    notes: String,
) -> Result<(), AppError> {
    invoices::update_draft(&db, id, customer_id, &lines, due_date.as_deref(), &notes).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_invoice_draft(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    invoices::delete_draft(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn issue_invoice(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    invoices::issue(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn void_invoice(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    invoices::void(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn record_payment(
    db: State<'_, Db>,
    invoice_id: i64,
    amount_minor: i64,
    method: String,
    reference: String,
) -> Result<i64, AppError> {
    Ok(payments::record_payment(&db, invoice_id, amount_minor, &method, &reference).await?)
}

#[tauri::command]
pub async fn invoice_payments(
    db: State<'_, Db>,
    invoice_id: i64,
) -> Result<Vec<Payment>, AppError> {
    Ok(payments::list_for_invoice(&db, invoice_id).await?)
}

#[tauri::command]
pub async fn delete_payment(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    payments::delete_payment(&db, id).await?;
    Ok(())
}
