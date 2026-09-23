//! Database backup and restore-file validation.
//!
//! Backups use `VACUUM INTO`, which writes a single, transactionally-consistent copy of the
//! database (safe even with WAL active) — far safer than copying the `.sqlite` file by hand.
//! Restore is orchestrated by the Tauri layer; this module provides the integrity check that
//! guards it.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Connection};

use crate::db::Db;
use crate::error::DataError;

static BACKUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Write a consistent single-file backup of the database to `dest` (`VACUUM INTO`).
pub async fn backup(db: &Db, dest: &Path) -> Result<(), DataError> {
    if dest.exists() {
        return Err(DataError::Other(
            "backup destination already exists; choose a new filename so the existing backup is preserved"
                .into(),
        ));
    }
    let dest = dest.to_string_lossy().into_owned();
    sqlx::query("VACUUM INTO ?").bind(dest).execute(db).await?;
    Ok(())
}

/// Write a backup to `dest`, replacing a file the user has chosen to overwrite. The copy is written
/// beside it first and then renamed over it, so an interrupted backup never leaves a half-written
/// file where a good one was.
pub async fn backup_replacing(db: &Db, dest: &Path) -> Result<(), DataError> {
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| DataError::Other("choose a file name for the backup".into()))?;
    let partial = dest.with_file_name(format!(".{file_name}.{}.partial", std::process::id()));
    let _ = std::fs::remove_file(&partial);
    if let Err(error) = backup(db, &partial).await {
        let _ = std::fs::remove_file(&partial);
        return Err(error);
    }
    std::fs::rename(&partial, dest).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        DataError::Other(format!("could not save the backup there: {e}"))
    })
}

/// Copy the database at `src` into a new file at `dest` through SQLite (`VACUUM INTO` from a
/// read-only connection), so committed changes still in `src`'s write-ahead log are included —
/// a plain file copy would silently drop them.
pub async fn copy_database(src: &Path, dest: &Path) -> Result<(), DataError> {
    if dest.exists() {
        return Err(DataError::Other(
            "the copy's destination already exists".into(),
        ));
    }
    let mut conn = SqliteConnectOptions::new()
        .filename(src)
        .read_only(true)
        .create_if_missing(false)
        .connect()
        .await?;
    let result = sqlx::query("VACUUM INTO ?")
        .bind(dest.to_string_lossy().into_owned())
        .execute(&mut conn)
        .await;
    conn.close().await?;
    result?;
    Ok(())
}

/// Flush every WAL frame into the main database file before that file is moved on its own.
pub async fn checkpoint(db: &Db) -> Result<(), DataError> {
    let (busy, _, _): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(db)
        .await?;
    if busy != 0 {
        return Err(DataError::Other(
            "database remained busy during checkpoint".into(),
        ));
    }
    Ok(())
}

/// A fresh, never-used backup path in `dir`: `{prefix}-{UTC timestamp}-{pid}-{sequence}.sqlite`.
/// UTC keeps names in true chronological order across daylight-saving changes and travel, which
/// rotation relies on. The process ID and an in-process sequence keep rapid or concurrent launches
/// from colliding.
pub async fn unique_backup_path(
    db: &Db,
    dir: &Path,
    prefix: &str,
) -> Result<std::path::PathBuf, DataError> {
    let stamp: String = sqlx::query_scalar("SELECT strftime('%Y%m%d-%H%M%f','now')")
        .fetch_one(db)
        .await?;
    let process_id = std::process::id();
    Ok(loop {
        let sequence = BACKUP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = dir.join(format!(
            "{prefix}-{stamp}-{process_id}-{sequence:04}.sqlite"
        ));
        if !candidate.exists() {
            break candidate;
        }
    })
}

/// Write a rotating automatic backup into `dir` and prune older ones (see [`prune_auto_backups`]).
/// Runs at app startup as cheap data-loss insurance for users who never press the manual backup
/// button.
pub async fn auto_backup(
    db: &Db,
    dir: &Path,
    keep: usize,
) -> Result<std::path::PathBuf, DataError> {
    std::fs::create_dir_all(dir).map_err(|e| DataError::Other(format!("backup dir: {e}")))?;
    let dest = unique_backup_path(db, dir, "auto").await?;
    backup(db, &dest).await?;
    prune_auto_backups(dir, keep, Some(&dest));
    Ok(dest)
}

/// How many distinct days keep their newest automatic backup, on top of the newest `keep` files.
const DAILY_BACKUP_DAYS: usize = 30;

/// Delete old `auto-*.sqlite` files. Kept: the newest `keep` files, the newest file from each of the
/// last 30 days that have backups, and `protect` (the backup just written). The daily tier means a
/// run of launches against a bad or empty database cannot rotate away every copy of the real data.
/// Best-effort: rotation must never take the app down.
pub fn prune_auto_backups(dir: &Path, keep: usize, protect: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut autos: Vec<(String, std::path::PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?.to_owned();
            (name.starts_with("auto-") && name.ends_with(".sqlite")).then_some((name, p))
        })
        .collect();
    // Timestamped names sort oldest-first; walk newest-first.
    autos.sort();
    autos.reverse();
    let mut days_kept: Vec<String> = Vec::new();
    for (index, (name, path)) in autos.iter().enumerate() {
        let day = name.get(5..13).unwrap_or_default().to_owned();
        let newest_of_day = !days_kept.contains(&day) && days_kept.len() < DAILY_BACKUP_DAYS;
        if newest_of_day {
            days_kept.push(day);
        }
        let protected = protect.is_some_and(|p| p == path.as_path());
        if index < keep || newest_of_day || protected {
            continue;
        }
        let _ = std::fs::remove_file(path);
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
    if result != "ok" {
        return Err(DataError::Other(format!(
            "file failed SQLite integrity check: {result}"
        )));
    }

    // Integrity alone is not enough: another application's SQLite file can be perfectly healthy
    // but would make RMB fail at startup after restore. Require RMB's marker and sqlx history.
    let marker =
        sqlx::query_scalar::<_, String>("SELECT value FROM app_meta WHERE key = 'schema_version'")
            .fetch_optional(&mut conn)
            .await
            .map_err(|_| DataError::Other("file is not an RMB backup".into()))?;
    if marker.as_deref().is_none_or(str::is_empty) {
        return Err(DataError::Other("file is not an RMB backup".into()));
    }
    let failed_migrations =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 0")
            .fetch_one(&mut conn)
            .await
            .map_err(|_| DataError::Other("file is not an RMB backup".into()))?;
    if failed_migrations != 0 {
        return Err(DataError::Other(
            "backup contains an incomplete database migration".into(),
        ));
    }
    conn.close().await?;
    Ok(())
}
