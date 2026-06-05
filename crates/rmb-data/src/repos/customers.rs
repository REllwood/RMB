//! Customer repository (soft-deleted; searchable by name/email).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::Db;
use crate::error::DataError;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub billing_address: String,
    pub notes: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomerInput {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub billing_address: String,
    pub notes: String,
}

pub async fn list(db: &Db, search: Option<&str>) -> Result<Vec<Customer>, DataError> {
    let like = format!("%{}%", search.unwrap_or("").trim());
    Ok(sqlx::query_as::<_, Customer>(
        "SELECT id, name, email, phone, billing_address, notes, created_at FROM customer \
         WHERE deleted_at IS NULL AND (name LIKE ?1 OR email LIKE ?1) ORDER BY name",
    )
    .bind(like)
    .fetch_all(db)
    .await?)
}

pub async fn get(db: &Db, id: i64) -> Result<Option<Customer>, DataError> {
    Ok(sqlx::query_as::<_, Customer>(
        "SELECT id, name, email, phone, billing_address, notes, created_at FROM customer \
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn create(db: &Db, input: &CustomerInput) -> Result<i64, DataError> {
    let id = sqlx::query(
        "INSERT INTO customer (name, email, phone, billing_address, notes) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&input.name)
    .bind(&input.email)
    .bind(&input.phone)
    .bind(&input.billing_address)
    .bind(&input.notes)
    .execute(db)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn update(db: &Db, id: i64, input: &CustomerInput) -> Result<(), DataError> {
    sqlx::query(
        "UPDATE customer SET name=?, email=?, phone=?, billing_address=?, notes=? \
         WHERE id=? AND deleted_at IS NULL",
    )
    .bind(&input.name)
    .bind(&input.email)
    .bind(&input.phone)
    .bind(&input.billing_address)
    .bind(&input.notes)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn soft_delete(db: &Db, id: i64) -> Result<(), DataError> {
    sqlx::query("UPDATE customer SET deleted_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str) -> CustomerInput {
        CustomerInput {
            name: name.into(),
            email: format!("{}@example.com", name.to_lowercase()),
            phone: "555".into(),
            billing_address: "1 St".into(),
            notes: String::new(),
        }
    }

    #[sqlx::test]
    async fn crud_search_and_soft_delete(pool: Db) -> Result<(), DataError> {
        let a = create(&pool, &input("Alice")).await?;
        create(&pool, &input("Bob")).await?;

        assert_eq!(list(&pool, None).await?.len(), 2);
        assert_eq!(list(&pool, Some("ali")).await?.len(), 1);

        update(
            &pool,
            a,
            &CustomerInput {
                name: "Alice Smith".into(),
                ..input("Alice")
            },
        )
        .await?;
        assert_eq!(get(&pool, a).await?.unwrap().name, "Alice Smith");

        soft_delete(&pool, a).await?;
        assert!(get(&pool, a).await?.is_none());
        assert_eq!(list(&pool, None).await?.len(), 1);
        Ok(())
    }
}
