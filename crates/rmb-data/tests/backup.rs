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

#[sqlx::test]
async fn auto_backup_writes_and_rotates(pool: Db) -> Result<(), DataError> {
    let dir = std::env::temp_dir().join(format!("rmb-autobk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // First run writes a valid, integrity-checked backup file.
    let written = backup::auto_backup(&pool, &dir, 7).await?;
    assert!(written.exists());
    backup::validate_backup(&written).await?;

    // Same-second relaunch is a no-op, not an error (VACUUM INTO can't overwrite).
    let again = backup::auto_backup(&pool, &dir, 7).await?;
    assert_eq!(written, again);

    // Rotation keeps only the newest N auto files (and ignores other files).
    for i in 0..9 {
        std::fs::write(dir.join(format!("auto-20200101-00000{i}.sqlite")), b"x").unwrap();
    }
    std::fs::write(dir.join("manual-keep.sqlite"), b"x").unwrap();
    backup::prune_auto_backups(&dir, 3);
    let autos = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("auto-"))
        .count();
    assert_eq!(autos, 3);
    assert!(
        dir.join("manual-keep.sqlite").exists(),
        "non-auto files untouched"
    );
    // The real (newest-named) backup survives pruning.
    assert!(written.exists());

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
