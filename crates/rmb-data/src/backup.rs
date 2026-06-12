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

/// Write a rotating automatic backup (`auto-YYYYMMDD-HHMMSS.sqlite`) into `dir`, keeping the
/// newest `keep` files. Runs at app startup as cheap data-loss insurance for users who never
/// press the manual backup button. Returns the path written (or already present this second).
pub async fn auto_backup(
    db: &Db,
    dir: &Path,
    keep: usize,
) -> Result<std::path::PathBuf, DataError> {
    std::fs::create_dir_all(dir).map_err(|e| DataError::Other(format!("backup dir: {e}")))?;
    let stamp: String = sqlx::query_scalar("SELECT strftime('%Y%m%d-%H%M%S','now','localtime')")
        .fetch_one(db)
        .await?;
    let dest = dir.join(format!("auto-{stamp}.sqlite"));
    if !dest.exists() {
        // VACUUM INTO refuses to overwrite; same-second relaunch just keeps the existing file.
        backup(db, &dest).await?;
    }
    prune_auto_backups(dir, keep);
    Ok(dest)
}

/// Delete all but the newest `keep` `auto-*.sqlite` files (timestamped names sort newest-last).
/// Best-effort: rotation must never take the app down.
pub fn prune_auto_backups(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut autos: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("auto-") && n.ends_with(".sqlite"))
        })
        .collect();
    autos.sort();
    if autos.len() > keep {
        let excess = autos.len() - keep;
        for old in &autos[..excess] {
            let _ = std::fs::remove_file(old);
        }
    }
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
