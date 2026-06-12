//! Job + timekeeping commands.

use rmb_data::db::Db;
use rmb_data::repos::jobs::{
    self as repo, Job, JobDetail, JobInput, JobMaterialInput, TimeEntryInput,
};
use tauri::State;

use crate::error::AppError;

#[tauri::command]
pub async fn list_jobs(db: State<'_, Db>) -> Result<Vec<Job>, AppError> {
    Ok(repo::list(&db).await?)
}

#[tauri::command]
pub async fn get_job(db: State<'_, Db>, id: i64) -> Result<Option<JobDetail>, AppError> {
    Ok(repo::get_detail(&db, id).await?)
}

#[tauri::command]
pub async fn create_job(db: State<'_, Db>, input: JobInput) -> Result<i64, AppError> {
    Ok(repo::create(&db, &input).await?)
}

#[tauri::command]
pub async fn convert_quote_to_job(db: State<'_, Db>, quote_id: i64) -> Result<i64, AppError> {
    Ok(repo::create_from_quote(&db, quote_id).await?)
}

#[tauri::command]
pub async fn set_job_status(db: State<'_, Db>, id: i64, status: String) -> Result<(), AppError> {
    repo::set_status(&db, id, &status).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_job(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    repo::delete(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn add_time_entry(
    db: State<'_, Db>,
    job_id: i64,
    entry: TimeEntryInput,
) -> Result<i64, AppError> {
    Ok(repo::add_time(&db, job_id, &entry).await?)
}

#[tauri::command]
pub async fn add_job_material(
    db: State<'_, Db>,
    job_id: i64,
    material: JobMaterialInput,
) -> Result<i64, AppError> {
    Ok(repo::add_material(&db, job_id, &material).await?)
}

#[tauri::command]
pub async fn delete_time_entry(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    repo::delete_time(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_job_material(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    repo::delete_material(&db, id).await?;
    Ok(())
}

#[tauri::command]
pub async fn invoice_job(db: State<'_, Db>, id: i64) -> Result<i64, AppError> {
    Ok(repo::invoice_from_job(&db, id).await?)
}
