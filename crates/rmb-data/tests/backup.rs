//! Integration tests for backup creation + restore-file validation.

use rmb_data::backup;
use rmb_data::db::{self, Db};
use rmb_data::repos::settings;
use rmb_data::DataError;

#[sqlx::test]
async fn backup_creates_a_valid_database(pool: Db) -> Result<(), DataError> {
    let dest = std::env::temp_dir().join(format!("rmb-backup-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&dest);

    settings::set_logo_asset(
        &pool,
        Some("/old-machine/logo.png"),
        Some((b"portable-logo", "png")),
    )
    .await?;
    backup::backup(&pool, &dest).await?;
    assert!(dest.exists(), "backup file should be written");

    // The backup itself must pass the same integrity gate restore uses.
    backup::validate_backup(&dest).await?;
    let restored = db::open(&dest).await?;
    assert_eq!(
        settings::get_logo_asset(&restored).await?.unwrap().data,
        b"portable-logo"
    );
    restored.close().await;

    let _ = std::fs::remove_file(&dest);
    Ok(())
}

#[sqlx::test]
async fn current_build_rejects_a_backup_with_future_migrations(pool: Db) -> Result<(), DataError> {
    use sqlx::{ConnectOptions, Connection};

    let dest = std::env::temp_dir().join(format!("rmb-future-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&dest);
    backup::backup(&pool, &dest).await?;

    let mut conn = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&dest)
        .connect()
        .await?;
    sqlx::query(
        "INSERT INTO _sqlx_migrations \
         (version, description, installed_on, success, checksum, execution_time) \
         VALUES (9999, 'future release', datetime('now'), 1, x'00', 0)",
    )
    .execute(&mut conn)
    .await?;
    conn.close().await?;

    assert!(
        db::open(&dest).await.is_err(),
        "restore staging must reject a database created by a newer incompatible build"
    );
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
async fn backup_never_overwrites_an_existing_file(pool: Db) -> Result<(), DataError> {
    let dest = std::env::temp_dir().join(format!("rmb-no-overwrite-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&dest);
    backup::backup(&pool, &dest).await?;
    let original_size = std::fs::metadata(&dest).unwrap().len();

    assert!(backup::backup(&pool, &dest).await.is_err());
    assert_eq!(std::fs::metadata(&dest).unwrap().len(), original_size);
    let _ = std::fs::remove_file(&dest);
    Ok(())
}

#[sqlx::test]
async fn validate_rejects_an_unrelated_but_healthy_sqlite_file(_pool: Db) -> Result<(), DataError> {
    use sqlx::{ConnectOptions, Connection};

    let unrelated =
        std::env::temp_dir().join(format!("rmb-unrelated-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&unrelated);
    let mut conn = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&unrelated)
        .create_if_missing(true)
        .connect()
        .await?;
    sqlx::query("CREATE TABLE other_app (id INTEGER PRIMARY KEY)")
        .execute(&mut conn)
        .await?;
    conn.close().await?;

    let result = backup::validate_backup(&unrelated).await;
    assert!(
        result.is_err(),
        "another app's SQLite file must be rejected"
    );
    let _ = std::fs::remove_file(&unrelated);
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

    // Rapid consecutive launches must each preserve a distinct point-in-time backup.
    let again = backup::auto_backup(&pool, &dir, 7).await?;
    assert_ne!(written, again);
    assert!(again.exists());
    backup::validate_backup(&again).await?;

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
