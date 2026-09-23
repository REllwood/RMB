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

fn normalise(input: &CustomerInput) -> Result<CustomerInput, DataError> {
    let value = CustomerInput {
        name: input.name.trim().to_owned(),
        email: input.email.trim().to_owned(),
        phone: input.phone.trim().to_owned(),
        billing_address: input.billing_address.trim().to_owned(),
        notes: input.notes.trim().to_owned(),
    };
    if value.name.is_empty() {
        return Err(DataError::Other("customer name is required".into()));
    }
    for (label, text, maximum) in [
        ("customer name", value.name.as_str(), 200),
        ("email", value.email.as_str(), 320),
        ("phone", value.phone.as_str(), 100),
        ("billing address", value.billing_address.as_str(), 2_000),
        ("customer notes", value.notes.as_str(), 10_000),
    ] {
        if text.chars().count() > maximum {
            return Err(DataError::Other(format!(
                "{label} cannot exceed {maximum} characters"
            )));
        }
    }
    Ok(value)
}

pub async fn list(db: &Db, search: Option<&str>) -> Result<Vec<Customer>, DataError> {
    let like = crate::repos::like_pattern(search);
    Ok(sqlx::query_as::<_, Customer>(
        "SELECT id, name, email, phone, billing_address, notes, \
         datetime(created_at, 'localtime') AS created_at FROM customer \
         WHERE deleted_at IS NULL AND (name LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\') \
         ORDER BY name",
    )
    .bind(like)
    .fetch_all(db)
    .await?)
}

/// A customer row for exports, including deleted customers (they may still owe money or appear on
/// past invoices).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CustomerExportRow {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub billing_address: String,
    pub notes: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

pub async fn export_rows(db: &Db) -> Result<Vec<CustomerExportRow>, DataError> {
    Ok(sqlx::query_as::<_, CustomerExportRow>(
        "SELECT name, email, phone, billing_address, notes, datetime(created_at, 'localtime') AS created_at, \
         datetime(deleted_at, 'localtime') AS deleted_at FROM customer \
         ORDER BY name, id",
    )
    .fetch_all(db)
    .await?)
}

/// A customer even if deleted — for documents that must still show who they were for.
pub async fn get_including_deleted(db: &Db, id: i64) -> Result<Option<Customer>, DataError> {
    Ok(sqlx::query_as::<_, Customer>(
        "SELECT id, name, email, phone, billing_address, notes, \
         datetime(created_at, 'localtime') AS created_at FROM customer \
         WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn get(db: &Db, id: i64) -> Result<Option<Customer>, DataError> {
    Ok(sqlx::query_as::<_, Customer>(
        "SELECT id, name, email, phone, billing_address, notes, \
         datetime(created_at, 'localtime') AS created_at FROM customer \
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn create(db: &Db, input: &CustomerInput) -> Result<i64, DataError> {
    let input = normalise(input)?;
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
    let input = normalise(input)?;
    let result = sqlx::query(
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
    if result.rows_affected() != 1 {
        return Err(DataError::Other("customer not found".into()));
    }
    Ok(())
}

/// Soft-delete a customer. Refused while they have anything still in progress or money still owed:
/// drafts, open quotes, jobs not yet invoiced, recurring schedules (active or paused), or issued
/// invoices with a balance. One statement, so nothing can be added in between the check and the
/// delete.
pub async fn soft_delete(db: &Db, id: i64) -> Result<(), DataError> {
    let result = sqlx::query(
        "UPDATE customer SET deleted_at = datetime('now') \
         WHERE id = ?1 AND deleted_at IS NULL \
           AND NOT EXISTS (SELECT 1 FROM invoice WHERE customer_id = ?1 \
                           AND status IN ('draft', 'issued', 'part_paid')) \
           AND NOT EXISTS (SELECT 1 FROM quote WHERE customer_id = ?1 AND deleted_at IS NULL \
                           AND status NOT IN ('declined', 'expired', 'converted')) \
           AND NOT EXISTS (SELECT 1 FROM job WHERE customer_id = ?1 AND deleted_at IS NULL \
                           AND status != 'invoiced') \
           AND NOT EXISTS (SELECT 1 FROM recurring_invoice WHERE customer_id = ?1)",
    )
    .bind(id)
    .execute(db)
    .await?;
    if result.rows_affected() == 1 {
        return Ok(());
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM customer WHERE id = ? AND deleted_at IS NULL)",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    if !exists {
        return Err(DataError::Other("customer not found".into()));
    }
    let blockers = deletion_blockers(db, id).await?;
    Err(DataError::Other(format!(
        "this customer can't be deleted while they have {}",
        blockers.join(", ")
    )))
}

/// Plain-language list of what stops a customer from being deleted.
async fn deletion_blockers(db: &Db, id: i64) -> Result<Vec<String>, DataError> {
    let (drafts, unpaid, quotes, jobs, schedules): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT COUNT(*) FROM invoice WHERE customer_id = ?1 AND status = 'draft'), \
           (SELECT COUNT(*) FROM invoice WHERE customer_id = ?1 AND status IN ('issued', 'part_paid')), \
           (SELECT COUNT(*) FROM quote WHERE customer_id = ?1 AND deleted_at IS NULL \
              AND status NOT IN ('declined', 'expired', 'converted')), \
           (SELECT COUNT(*) FROM job WHERE customer_id = ?1 AND deleted_at IS NULL \
              AND status != 'invoiced'), \
           (SELECT COUNT(*) FROM recurring_invoice WHERE customer_id = ?1)",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    let mut blockers = Vec::new();
    for (count, one, many) in [
        (drafts, "a draft invoice", "draft invoices"),
        (unpaid, "an unpaid invoice", "unpaid invoices"),
        (quotes, "an open quote", "open quotes"),
        (
            jobs,
            "a job that isn't invoiced",
            "jobs that aren't invoiced",
        ),
        (schedules, "a recurring schedule", "recurring schedules"),
    ] {
        match count {
            0 => {}
            1 => blockers.push(one.to_owned()),
            n => blockers.push(format!("{n} {many}")),
        }
    }
    if blockers.is_empty() {
        blockers.push("work in progress".into());
    }
    Ok(blockers)
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

    #[sqlx::test]
    async fn validates_and_normalises_input(pool: Db) -> Result<(), DataError> {
        assert!(create(&pool, &input("   ")).await.is_err());
        let id = create(&pool, &input("  Alice  ")).await?;
        assert_eq!(get(&pool, id).await?.unwrap().name, "Alice");
        assert!(update(&pool, 99_999, &input("Missing")).await.is_err());
        assert!(soft_delete(&pool, 99_999).await.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn active_work_prevents_customer_deletion(pool: Db) -> Result<(), DataError> {
        let customer = create(&pool, &input("Alice")).await?;
        sqlx::query("INSERT INTO job (customer_id, title) VALUES (?, 'Active job')")
            .bind(customer)
            .execute(&pool)
            .await?;

        assert!(soft_delete(&pool, customer).await.is_err());
        assert!(get(&pool, customer).await?.is_some());
        Ok(())
    }
}
