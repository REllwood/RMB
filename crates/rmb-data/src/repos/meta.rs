//! `app_meta` key/value repository — schema version + small app/UI preferences (e.g. theme).

use crate::db::Db;
use crate::error::DataError;

/// Fetch a meta value by key (`None` if absent).
pub async fn get(db: &Db, key: &str) -> Result<Option<String>, DataError> {
    let value = sqlx::query_scalar::<_, String>("SELECT value FROM app_meta WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(value)
}

/// Insert or update a meta value (upsert).
pub async fn set(db: &Db, key: &str, value: &str) -> Result<(), DataError> {
    sqlx::query(
        "INSERT INTO app_meta (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await?;
    Ok(())
}
