//! Integration tests for the `meta` repository, each against a fresh migrated SQLite file
//! (`#[sqlx::test]` provisions and tears down the DB automatically).

use rmb_data::db::Db;
use rmb_data::repos::meta;
use rmb_data::DataError;

#[sqlx::test]
async fn set_then_get_roundtrips(pool: Db) -> Result<(), DataError> {
    assert_eq!(meta::get(&pool, "theme").await?, None);

    meta::set(&pool, "theme", "dark").await?;
    assert_eq!(meta::get(&pool, "theme").await?, Some("dark".to_string()));

    // upsert overwrites
    meta::set(&pool, "theme", "light").await?;
    assert_eq!(meta::get(&pool, "theme").await?, Some("light".to_string()));
    Ok(())
}

#[sqlx::test]
async fn migration_seeds_schema_version(pool: Db) -> Result<(), DataError> {
    assert_eq!(
        meta::get(&pool, "schema_version").await?,
        Some("1".to_string())
    );
    Ok(())
}
