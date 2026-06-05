//! Catalog (items) + stock commands.

use rmb_data::db::Db;
use rmb_data::repos::items::{self, Item, ItemInput, StockMovement};
use tauri::State;

use crate::error::AppError;

#[tauri::command]
pub async fn list_items(db: State<'_, Db>, search: Option<String>) -> Result<Vec<Item>, AppError> {
    Ok(items::list(&db, search.as_deref()).await?)
}

#[tauri::command]
pub async fn get_item(db: State<'_, Db>, id: i64) -> Result<Option<Item>, AppError> {
    Ok(items::get(&db, id).await?)
}

#[tauri::command]
pub async fn create_item(db: State<'_, Db>, input: ItemInput) -> Result<i64, AppError> {
    Ok(items::create(&db, &input).await?)
}

#[tauri::command]
pub async fn update_item(db: State<'_, Db>, id: i64, input: ItemInput) -> Result<(), AppError> {
    items::update(&db, id, &input).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_item(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    items::soft_delete(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn adjust_stock(
    db: State<'_, Db>,
    item_id: i64,
    qty_delta: i64,
    note: String,
) -> Result<(), AppError> {
    items::adjust_stock(&db, item_id, qty_delta, &note).await?;
    Ok(())
}

#[tauri::command]
pub async fn item_movements(
    db: State<'_, Db>,
    item_id: i64,
) -> Result<Vec<StockMovement>, AppError> {
    Ok(items::movements(&db, item_id).await?)
}
