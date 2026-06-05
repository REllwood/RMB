//! Integration tests for backup creation + restore-file validation.

use rmb_data::backup;
use rmb_data::db::Db;
use rmb_data::DataError;

#[sqlx::test]
async fn backup_creates_a_valid_database(pool: Db) -> Result<(), DataError> {
    let dest = std::env::temp_dir().join(format!("rmb-backup-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&dest);

    backup::backup(&pool, &dest).await?;
    assert!(dest.exists(), "backup file should be written");

    // The backup itself must pass the same integrity gate restore uses.
    backup::validate_backup(&dest).await?;

    let _ = std::fs::remove_file(&dest);
    Ok(())
}

#[sqlx::test]
async fn validate_rejects_a_non_database_file(_pool: Db) -> Result<(), DataError> {
    let bogus = std::env::temp_dir().join(format!("rmb-bogus-{}.bin", std::process::id()));
    std::fs::write(&bogus, b"definitely not a sqlite database").unwrap();

    let result = backup::validate_backup(&bogus).await;
    assert!(result.is_err(), "a non-database file must fail validation");

    let _ = std::fs::remove_file(&bogus);
    Ok(())
}
