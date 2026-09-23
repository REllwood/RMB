//! SQLite connection pool, pragmas, and migrations for RMB.
//!
//! [`connect_options`] is shared by the application and the test harness so tests exercise
//! the same pragmas (WAL, foreign keys, busy timeout) as production. Migrations are embedded
//! into the binary and run automatically on [`open`] and [`open_app`].

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::error::DataError;

/// The application's database handle (a pooled SQLite connection).
pub type Db = SqlitePool;

/// Embedded, ordered, checksum-verified migrations. Never edit an applied migration — add a
/// new one (a checksum mismatch hard-fails on users' machines).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Connection options shared by app and tests so both run the same pragmas.
pub fn connect_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
}

async fn connect(path: &Path) -> Result<Db, DataError> {
    Ok(SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options(path))
        .await?)
}

/// Open a pooled connection at `path` and apply all pending migrations.
pub async fn open(path: &Path) -> Result<Db, DataError> {
    let pool = connect(path).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// The application database after [`open_app`].
#[derive(Debug)]
pub struct OpenedDb {
    pub db: Db,
    /// True when no database file existed and an empty one was created.
    pub created: bool,
    /// Copy of the database taken before this launch applied pending migrations to it.
    pub pre_upgrade_backup: Option<PathBuf>,
}

/// Open the application database, first copying an existing database into `backup_dir` when this
/// build has migrations it has not seen yet. A failed upgrade then always leaves a restorable copy
/// of the last good state, which the rotating launch backups (taken after migrations) cannot.
pub async fn open_app(path: &Path, backup_dir: &Path) -> Result<OpenedDb, DataError> {
    let existed = path.exists();
    let pool = connect(path).await?;
    let mut pre_upgrade_backup = None;
    if existed && has_pending_migrations(&pool).await? {
        std::fs::create_dir_all(backup_dir)
            .map_err(|e| DataError::Other(format!("could not create the backups folder: {e}")))?;
        let dest = crate::backup::unique_backup_path(&pool, backup_dir, "pre-upgrade").await?;
        crate::backup::backup(&pool, &dest).await?;
        pre_upgrade_backup = Some(dest);
    }
    if let Err(error) = MIGRATOR.run(&pool).await {
        pool.close().await;
        let saved = pre_upgrade_backup
            .as_deref()
            .map(|p| {
                format!(
                    " A copy from before the upgrade was saved to {}.",
                    p.display()
                )
            })
            .unwrap_or_default();
        return Err(DataError::Other(format!(
            "the database could not be upgraded for this version of RMB ({error}).{saved}"
        )));
    }
    Ok(OpenedDb {
        db: pool,
        created: !existed,
        pre_upgrade_backup,
    })
}

async fn has_pending_migrations(db: &Db) -> Result<bool, DataError> {
    let tracked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(db)
    .await?;
    if !tracked {
        return Ok(true);
    }
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success = 1")
            .fetch_all(db)
            .await?;
    Ok(MIGRATOR.iter().any(|m| !applied.contains(&m.version)))
}

/// Begin a write transaction. `BEGIN IMMEDIATE` takes SQLite's write lock up front (waiting up to
/// the busy timeout), so a transaction that reads before it writes can never fail part-way with
/// "database is locked" because another write committed first.
pub async fn begin_write(db: &Db) -> Result<sqlx::Transaction<'static, sqlx::Sqlite>, DataError> {
    Ok(db.begin_with("BEGIN IMMEDIATE").await?)
}

/// Today as `YYYY-MM-DD` in the machine's local timezone (business dates are local, not UTC).
pub async fn today_local(db: &Db) -> Result<String, DataError> {
    Ok(sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(db)
        .await?)
}
