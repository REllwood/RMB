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

static AUTO_BACKUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

/// Write a uniquely named rotating automatic backup into `dir`, keeping the newest `keep` files.
/// Runs at app startup as cheap data-loss insurance for users who never press the manual backup
/// button. Milliseconds, the process ID, and an in-process sequence prevent rapid or concurrent
/// launches from reusing an older backup filename.
pub async fn auto_backup(
    db: &Db,
    dir: &Path,
    keep: usize,
) -> Result<std::path::PathBuf, DataError> {
    std::fs::create_dir_all(dir).map_err(|e| DataError::Other(format!("backup dir: {e}")))?;
    let stamp: String = sqlx::query_scalar("SELECT strftime('%Y%m%d-%H%M%f','now','localtime')")
        .fetch_one(db)
        .await?;
    let process_id = std::process::id();
    let dest = loop {
        let sequence = AUTO_BACKUP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = dir.join(format!("auto-{stamp}-{process_id}-{sequence:04}.sqlite"));
        if !candidate.exists() {
            break candidate;
        }
    };
    backup(db, &dest).await?;
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
