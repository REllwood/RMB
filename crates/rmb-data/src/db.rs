//! SQLite connection pool, pragmas, and migrations for RMB.
//!
//! [`connect_options`] is shared by the application and the test harness so tests exercise
//! the same pragmas (WAL, foreign keys, busy timeout) as production. Migrations are embedded
//! into the binary and run automatically on [`open`].

use std::path::Path;
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

/// Open a pooled connection at `path` and apply all pending migrations.
pub async fn open(path: &Path) -> Result<Db, DataError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options(path))
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}
