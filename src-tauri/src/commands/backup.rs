//! Backup / restore commands.
//!
//! Backup writes a `VACUUM INTO` snapshot to a user-chosen path. Restore validates the chosen
//! file is an intact SQLite database, closes the live pool, swaps the file in (dropping stale
//! WAL/SHM sidecars), and restarts the app so the new process opens the restored database.

use std::fs;
use std::path::{Path, PathBuf};

use rmb_data::backup as db_backup;
use rmb_data::db::Db;
use tauri::{AppHandle, Manager, State};

use crate::error::AppError;

/// Write a consistent backup of the database to `dest`.
#[tauri::command]
pub async fn backup_database(db: State<'_, Db>, dest: String) -> Result<(), AppError> {
    db_backup::backup(&db, Path::new(&dest)).await?;
    Ok(())
}

/// Restore the database from `src` (validated first), then restart the app.
#[tauri::command]
pub async fn restore_database(
    app: AppHandle,
    db: State<'_, Db>,
    src: String,
) -> Result<(), AppError> {
    let src = PathBuf::from(&src);

    // 1. Refuse anything that isn't an intact SQLite database.
    db_backup::validate_backup(&src).await?;

    // 2. Close the live pool so the file is no longer held open.
    db.close().await;

    // 3. Overwrite the live database and drop stale WAL/SHM sidecars.
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    let db_path = data_dir.join("rmb.sqlite");
    fs::copy(&src, &db_path).map_err(|e| AppError::Message(e.to_string()))?;
    let _ = fs::remove_file(data_dir.join("rmb.sqlite-wal"));
    let _ = fs::remove_file(data_dir.join("rmb.sqlite-shm"));

    // 4. Restart so a fresh process opens the restored database. `restart` diverges.
    app.restart();
}
