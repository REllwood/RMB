//! Backup / restore commands.
//!
//! Backup writes a `VACUUM INTO` snapshot to a user-chosen path. Restore validates the chosen
//! file is an intact SQLite database, closes the live pool, swaps the file in (dropping stale
//! WAL/SHM sidecars), and restarts the app so the new process opens the restored database.

use std::fs;
use std::path::{Path, PathBuf};

use rmb_data::backup as db_backup;
use rmb_data::db::{self, Db};
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

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::create_dir_all(&data_dir).map_err(|e| AppError::Message(e.to_string()))?;
    let db_path = data_dir.join("rmb.sqlite");
    if src.canonicalize().ok() == db_path.canonicalize().ok() {
        return Err(AppError::Message(
            "the selected file is already the live RMB database".into(),
        ));
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();

    // Stage under a unique name so sidecars from an interrupted older attempt can never attach to
    // this restore. Validate, migrate, and checkpoint the copy while the live pool is still open:
    // any incompatible future schema or checksum mismatch fails without touching live data.
    let staged = data_dir.join(format!(
        "rmb.restore.pending-{stamp}-{}.sqlite",
        std::process::id()
    ));
    fs::copy(&src, &staged).map_err(|e| AppError::Message(format!("stage restore: {e}")))?;
    db_backup::validate_backup(&staged).await?;
    let staged_pool = db::open(&staged).await.map_err(|e| {
        AppError::Message(format!("backup is not compatible with this RMB build: {e}"))
    })?;
    db_backup::checkpoint(&staged_pool)
        .await
        .map_err(|e| AppError::Message(format!("could not finalise the staged backup: {e}")))?;
    staged_pool.close().await;
    db_backup::validate_backup(&staged).await?;

    // Keep a transactionally-consistent pre-restore recovery copy alongside automatic backups.
    let safety_dir = data_dir.join("backups");
    let safety = db_backup::auto_backup(&db, &safety_dir, 7).await?;

    // 2. Close the live pool so the file is no longer held open.
    db.close().await;

    // 3. Preserve old WAL/SHM sidecars before replacing the main file, so they can never replay
    // over the restored snapshot if the process is interrupted between these steps.
    for suffix in ["wal", "shm"] {
        let sidecar = data_dir.join(format!("rmb.sqlite-{suffix}"));
        if sidecar.exists() {
            let preserved = safety_dir.join(format!("pre-restore-{stamp}.sqlite-{suffix}"));
            if let Err(error) = fs::rename(&sidecar, &preserved) {
                eprintln!(
                    "restore cancelled: could not preserve database {suffix} sidecar: {error}"
                );
                app.restart();
            }
        }
    }

    // 4. Swap same-filesystem paths so the replacement is never a partially-copied SQLite file.
    // Keeping the former live file also gives the user a second, directly recoverable copy.
    let previous_live = safety_dir.join(format!("pre-restore-{stamp}.sqlite"));
    if let Err(error) = fs::rename(&db_path, &previous_live) {
        eprintln!("restore cancelled: could not preserve the live database: {error}");
        app.restart();
    }
    if let Err(error) = fs::rename(&staged, &db_path) {
        eprintln!("restore failed while installing the staged database: {error}");
        if let Err(rollback_error) = fs::rename(&previous_live, &db_path) {
            eprintln!(
                "restore rollback failed ({rollback_error}); copying safety backup from {}",
                safety.display()
            );
            let _ = fs::copy(&safety, &db_path);
        }
        app.restart();
    }

    // 5. Restart so a fresh process opens the restored database. `restart` diverges.
    app.restart();
}
