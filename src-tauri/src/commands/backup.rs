//! Backup / restore commands.
//!
//! Backup writes a `VACUUM INTO` snapshot to a user-chosen path. Restore validates the chosen
//! file, copies it through SQLite into a staged file (so committed changes still in its
//! write-ahead log are kept), migrates and re-validates the copy while the live database is still
//! open, then swaps it in and restarts. Every failure is recorded so the next launch can say what
//! happened — a restart otherwise looks the same whether the restore worked or not.

use std::fs;
use std::path::{Path, PathBuf};

use rmb_data::backup as db_backup;
use rmb_data::db::{self, Db};
use tauri::{AppHandle, Manager, State};

use crate::error::AppError;

/// Written before a restore restarts the app and shown (then removed) at the next launch.
pub const RESTORE_OUTCOME_FILE: &str = "restore-outcome.txt";

fn data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::create_dir_all(&dir).map_err(|e| AppError::Message(e.to_string()))?;
    Ok(dir)
}

/// Require the extension the save dialog asked for, so a path can only ever produce the kind of
/// file the user chose to save.
pub fn ensure_extension(path: &str, allowed: &[&str]) -> Result<(), AppError> {
    let extension = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if allowed.contains(&extension.as_str()) {
        Ok(())
    } else {
        Err(AppError::Message(format!(
            "save the file with a .{} extension",
            allowed[0]
        )))
    }
}

/// The folder holding automatic and pre-upgrade backups, for display and the restore picker.
#[tauri::command]
pub fn backup_folder(app: AppHandle) -> Result<String, AppError> {
    Ok(data_dir(&app)?
        .join("backups")
        .to_string_lossy()
        .into_owned())
}

/// Write a consistent backup of the database to `dest`, replacing a file the user chose to
/// overwrite.
#[tauri::command]
pub async fn backup_database(
    app: AppHandle,
    db: State<'_, Db>,
    dest: String,
) -> Result<(), AppError> {
    ensure_extension(&dest, &["sqlite", "db"])?;
    let dest = PathBuf::from(dest);
    let live = data_dir(&app)?.join("rmb.sqlite");
    if dest.canonicalize().ok().is_some() && dest.canonicalize().ok() == live.canonicalize().ok() {
        return Err(AppError::Message(
            "choose a different file — that is RMB's live database".into(),
        ));
    }
    db_backup::backup_replacing(&db, &dest).await?;
    Ok(())
}

fn remove_with_sidecars(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}

fn record_outcome(data_dir: &Path, message: &str) {
    if let Err(error) = fs::write(data_dir.join(RESTORE_OUTCOME_FILE), message) {
        eprintln!("could not record the restore outcome: {error}");
    }
}

/// Retry a rename briefly: antivirus and indexers on Windows can hold a just-written file open.
fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < 10 => {
                attempt += 1;
                eprintln!("rename {} failed ({error}); retrying", from.display());
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(error) => return Err(error),
        }
    }
}

/// Validate, stage and migrate a copy of `src` without touching live data. Returns the staged
/// file, or removes whatever was staged and returns the error.
async fn stage(src: &Path, staged: &Path) -> Result<(), AppError> {
    db_backup::validate_backup(src).await?;
    db_backup::copy_database(src, staged).await.map_err(|e| {
        AppError::Message(format!("could not read the backup: {}", e.user_message()))
    })?;
    let staged_pool = db::open(staged).await.map_err(|e| {
        AppError::Message(format!(
            "this backup can't be used with this version of RMB ({})",
            e.user_message()
        ))
    })?;
    let checkpoint = db_backup::checkpoint(&staged_pool).await;
    staged_pool.close().await;
    checkpoint.map_err(|e| {
        AppError::Message(format!(
            "could not finalise the backup copy: {}",
            e.user_message()
        ))
    })?;
    db_backup::validate_backup(staged).await?;
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
    let data_dir = data_dir(&app)?;
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

    // 1. Stage under a unique name while the live database is untouched and still open.
    let staged = data_dir.join(format!(
        "rmb.restore.pending-{stamp}-{}.sqlite",
        std::process::id()
    ));
    if let Err(error) = stage(&src, &staged).await {
        remove_with_sidecars(&staged);
        return Err(error);
    }

    // 2. Keep a transactionally-consistent copy of the current data before replacing it.
    let safety_dir = data_dir.join("backups");
    if let Err(error) = db_backup::auto_backup(&db, &safety_dir, 7).await {
        remove_with_sidecars(&staged);
        return Err(AppError::Message(format!(
            "restore cancelled: could not back up the current data first ({})",
            error.user_message()
        )));
    }

    // 3. Close the live pool; a clean close checkpoints and removes its -wal/-shm files.
    db.close().await;

    // From here the app restarts on every path, so each outcome is written down first.
    let outcome = swap_in(&data_dir, &db_path, &staged, &safety_dir, stamp);
    match &outcome {
        Ok(()) => record_outcome(&data_dir, "Your backup was restored."),
        Err(message) => {
            eprintln!("restore failed: {message}");
            record_outcome(&data_dir, message);
        }
    }
    remove_with_sidecars(&staged);
    app.restart();
}

/// Move the live files aside and the staged copy into place. On failure the live database is put
/// back, and the returned message explains what the user still has.
fn swap_in(
    data_dir: &Path,
    db_path: &Path,
    staged: &Path,
    safety_dir: &Path,
    stamp: u128,
) -> Result<(), String> {
    // Leftover sidecars (another process still had the database open) would replay over the
    // restored copy; keep them with the previous database instead.
    for suffix in ["wal", "shm"] {
        let sidecar = data_dir.join(format!("rmb.sqlite-{suffix}"));
        if sidecar.exists() {
            let preserved = safety_dir.join(format!("pre-restore-{stamp}.sqlite-{suffix}"));
            rename_with_retry(&sidecar, &preserved).map_err(|error| {
                format!(
                    "The backup was not restored: RMB's database is still in use ({error}). Close \
                     any other RMB window and try again. Your data is unchanged."
                )
            })?;
        }
    }
    let previous = safety_dir.join(format!("pre-restore-{stamp}.sqlite"));
    rename_with_retry(db_path, &previous).map_err(|error| {
        format!(
            "The backup was not restored: the current database could not be moved aside \
             ({error}). Your data is unchanged."
        )
    })?;
    if let Err(error) = rename_with_retry(staged, db_path) {
        return match rename_with_retry(&previous, db_path) {
            Ok(()) => Err(format!(
                "The backup was not restored ({error}). Your previous data was put back unchanged."
            )),
            Err(rollback) => Err(format!(
                "The backup was not restored ({error}) and the previous database could not be put \
                 back ({rollback}). It is saved at {}; restore it from Settings.",
                previous.display()
            )),
        };
    }
    Ok(())
}
