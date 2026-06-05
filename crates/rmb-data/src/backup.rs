//! Database backup and restore-file validation.
//!
//! Backups use `VACUUM INTO`, which writes a single, transactionally-consistent copy of the
//! database (safe even with WAL active) — far safer than copying the `.sqlite` file by hand.
//! Restore is orchestrated by the Tauri layer; this module provides the integrity check that
//! guards it.

use std::path::Path;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};

use crate::db::Db;
use crate::error::DataError;

/// Write a consistent single-file backup of the database to `dest` (`VACUUM INTO`).
pub async fn backup(db: &Db, dest: &Path) -> Result<(), DataError> {
    let dest = dest.to_string_lossy().into_owned();
    sqlx::query("VACUUM INTO ?").bind(dest).execute(db).await?;
    Ok(())
}

/// Validate that `path` is a readable SQLite database that passes `PRAGMA integrity_check`.
/// Used to guard restore so a corrupt or non-database file can never overwrite live data.
pub async fn validate_backup(path: &Path) -> Result<(), DataError> {
    if !path.exists() {
        return Err(DataError::Other("backup file does not exist".into()));
    }
    let mut conn = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false)
        .connect()
        .await?;
    let result: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut conn)
        .await?;
    conn.close().await?;
    if result != "ok" {
        return Err(DataError::Other(format!(
            "file failed SQLite integrity check: {result}"
        )));
    }
    Ok(())
}
