//! Customer commands.

use rmb_data::db::Db;
use rmb_data::repos::customers::{self, Customer, CustomerInput};
use tauri::State;

use crate::error::AppError;

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
