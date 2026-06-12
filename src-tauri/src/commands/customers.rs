//! Customer commands.

use rmb_data::db::Db;
use rmb_data::repos::customers::{self, Customer, CustomerInput};
use rmb_data::repos::invoices::InvoiceRow;
use rmb_data::repos::jobs::Job;
use rmb_data::repos::quotes::QuoteRow;
use rmb_data::repos::{invoices, jobs, quotes};
use serde::Serialize;
use tauri::State;

use crate::error::AppError;

/// Everything for one customer in one place — quotes, jobs, invoices.
#[derive(Debug, Serialize)]
pub struct CustomerHistory {
    pub quotes: Vec<QuoteRow>,
    pub jobs: Vec<Job>,
    pub invoices: Vec<InvoiceRow>,
}

#[tauri::command]
pub async fn list_customers(
    db: State<'_, Db>,
    search: Option<String>,
) -> Result<Vec<Customer>, AppError> {
    Ok(customers::list(&db, search.as_deref()).await?)
}

#[tauri::command]
pub async fn get_customer(db: State<'_, Db>, id: i64) -> Result<Option<Customer>, AppError> {
    Ok(customers::get(&db, id).await?)
}

#[tauri::command]
pub async fn create_customer(db: State<'_, Db>, input: CustomerInput) -> Result<i64, AppError> {
    Ok(customers::create(&db, &input).await?)
}

#[tauri::command]
pub async fn update_customer(
    db: State<'_, Db>,
    id: i64,
    input: CustomerInput,
) -> Result<(), AppError> {
    customers::update(&db, id, &input).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_customer(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    customers::soft_delete(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn customer_history(db: State<'_, Db>, id: i64) -> Result<CustomerHistory, AppError> {
    Ok(CustomerHistory {
        quotes: quotes::list_for_customer(&db, id).await?,
        jobs: jobs::list_for_customer(&db, id).await?,
        invoices: invoices::list_for_customer(&db, id).await?,
    })
}
