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

    // Rotation keeps the newest N auto files plus the newest file of each earlier day, and ignores
    // other files. Nine same-day files on 2020-01-01, one on each of two earlier days.
    for i in 0..9 {
        std::fs::write(dir.join(format!("auto-20200101-00000{i}.sqlite")), b"x").unwrap();
    }
    std::fs::write(dir.join("auto-20191230-120000.sqlite"), b"x").unwrap();
    std::fs::write(dir.join("auto-20191231-080000.sqlite"), b"x").unwrap();
    std::fs::write(dir.join("auto-20191231-090000.sqlite"), b"x").unwrap();
    std::fs::write(dir.join("manual-keep.sqlite"), b"x").unwrap();
    backup::prune_auto_backups(&dir, 3, None);
    let mut autos: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("auto-"))
        .collect();
    autos.sort();
    // The two real backups (named with today's date) and the newest 2020-01-01 file fill the
    // newest-3 tier; each earlier day keeps exactly its newest file.
    assert!(autos.contains(&"auto-20191230-120000.sqlite".to_string()));
    assert!(autos.contains(&"auto-20191231-090000.sqlite".to_string()));
    assert!(!autos.contains(&"auto-20191231-080000.sqlite".to_string()));
    assert!(
        dir.join("manual-keep.sqlite").exists(),
        "non-auto files untouched"
    );
    assert!(written.exists() && again.exists());

    // The file just written is never pruned, even if a wrong clock gives it the oldest name.
    let old_named = dir.join("auto-19990101-000000.sqlite");
    std::fs::write(&old_named, b"x").unwrap();
    for day in 1..=40 {
        std::fs::write(dir.join(format!("auto-2021{:04}-000000.sqlite", day)), b"x").unwrap();
    }
    backup::prune_auto_backups(&dir, 3, Some(&old_named));
    assert!(
        old_named.exists(),
        "the backup just written must survive rotation"
    );

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[sqlx::test]
async fn a_chosen_backup_file_is_replaced_safely(pool: Db) -> Result<(), DataError> {
    let dest = std::env::temp_dir().join(format!("rmb-replace-{}.sqlite", std::process::id()));
    std::fs::write(&dest, b"an older file the user chose to overwrite").unwrap();
    backup::backup_replacing(&pool, &dest).await?;
    backup::validate_backup(&dest).await?;
    let _ = std::fs::remove_file(&dest);
    Ok(())
}

#[tokio::test]
async fn copying_a_live_database_includes_uncheckpointed_changes() -> Result<(), DataError> {
    let dir = std::env::temp_dir().join(format!("rmb-walcopy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let live = dir.join("rmb.sqlite");
    let pool = db::open(&live).await?;
    // Keep writing without closing, so the changes sit in the -wal file beside the database.
    for i in 0..25 {
        sqlx::query("INSERT INTO customer (name) VALUES (?)")
            .bind(format!("Customer {i}"))
            .execute(&pool)
            .await?;
    }
    let copy = dir.join("staged.sqlite");
    backup::copy_database(&live, &copy).await?;
    let staged = db::open(&copy).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM customer")
        .fetch_one(&staged)
        .await?;
    assert_eq!(count, 25, "a plain file copy would have lost these");
    staged.close().await;
    pool.close().await;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
