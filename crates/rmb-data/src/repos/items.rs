//! Catalog (products & services) + inventory (append-only movement ledger + cached on-hand).

use rmb_domain::inventory::MovementReason;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection};

use crate::db::Db;
use crate::error::DataError;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Item {
    pub id: i64,
    pub kind: String, // "product" | "service"
    pub name: String,
    pub sku: String,
    pub unit: String,
    pub default_price_minor: i64,
    pub default_tax_rate_id: Option<i64>,
    pub tracked: bool,
    pub qty_on_hand: i64,
    pub reorder_point: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemInput {
    pub kind: String,
    pub name: String,
    pub sku: String,
    pub unit: String,
    pub default_price_minor: i64,
    pub default_tax_rate_id: Option<i64>,
    pub tracked: bool,
    pub reorder_point: Option<i64>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct StockMovement {
    pub id: i64,
    pub qty_delta: i64,
    pub reason: String,
    pub occurred_at: String,
    pub note: String,
}

pub async fn list(db: &Db, search: Option<&str>) -> Result<Vec<Item>, DataError> {
    let like = format!("%{}%", search.unwrap_or("").trim());
    Ok(sqlx::query_as::<_, Item>(
        "SELECT id, kind, name, sku, unit, default_price_minor, default_tax_rate_id, tracked, \
         qty_on_hand, reorder_point FROM item WHERE deleted_at IS NULL \
         AND (name LIKE ?1 OR sku LIKE ?1) ORDER BY name",
    )
    .bind(like)
    .fetch_all(db)
    .await?)
}

pub async fn get(db: &Db, id: i64) -> Result<Option<Item>, DataError> {
    Ok(sqlx::query_as::<_, Item>(
        "SELECT id, kind, name, sku, unit, default_price_minor, default_tax_rate_id, tracked, \
         qty_on_hand, reorder_point FROM item WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn create(db: &Db, input: &ItemInput) -> Result<i64, DataError> {
    let id = sqlx::query(
        "INSERT INTO item (kind, name, sku, unit, default_price_minor, default_tax_rate_id, tracked, reorder_point) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&input.kind)
    .bind(&input.name)
    .bind(&input.sku)
    .bind(&input.unit)
    .bind(input.default_price_minor)
    .bind(input.default_tax_rate_id)
    .bind(input.tracked)
    .bind(input.reorder_point)
    .execute(db)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn update(db: &Db, id: i64, input: &ItemInput) -> Result<(), DataError> {
    // qty_on_hand is never set directly — only via stock movements.
    sqlx::query(
        "UPDATE item SET kind=?, name=?, sku=?, unit=?, default_price_minor=?, default_tax_rate_id=?, \
         tracked=?, reorder_point=? WHERE id=? AND deleted_at IS NULL",
    )
    .bind(&input.kind)
    .bind(&input.name)
    .bind(&input.sku)
    .bind(&input.unit)
    .bind(input.default_price_minor)
    .bind(input.default_tax_rate_id)
    .bind(input.tracked)
    .bind(input.reorder_point)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn soft_delete(db: &Db, id: i64) -> Result<(), DataError> {
    sqlx::query("UPDATE item SET deleted_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Apply a stock movement (ledger row + cached `qty_on_hand`) on the given connection, so callers
/// (e.g. invoice issue) can keep it in their own transaction. **Both writes in one transaction.**
pub(crate) async fn apply_movement(
    conn: &mut SqliteConnection,
    item_id: i64,
    qty_delta: i64,
    reason: MovementReason,
    ref_type: Option<&str>,
    ref_id: Option<i64>,
    note: &str,
) -> Result<(), DataError> {
    sqlx::query(
        "INSERT INTO stock_movement (item_id, qty_delta, reason, ref_type, ref_id, note) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(item_id)
    .bind(qty_delta)
    .bind(reason.as_db())
    .bind(ref_type)
    .bind(ref_id)
    .bind(note)
    .execute(&mut *conn)
    .await?;
    sqlx::query("UPDATE item SET qty_on_hand = qty_on_hand + ? WHERE id = ?")
        .bind(qty_delta)
        .bind(item_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Manual stock adjustment (its own transaction).
pub async fn adjust_stock(
    db: &Db,
    item_id: i64,
    qty_delta: i64,
    note: &str,
) -> Result<(), DataError> {
    let mut tx = db.begin().await?;
    apply_movement(
        &mut tx,
        item_id,
        qty_delta,
        MovementReason::Adjustment,
        Some("adjustment"),
        None,
        note,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn movements(db: &Db, item_id: i64) -> Result<Vec<StockMovement>, DataError> {
    Ok(sqlx::query_as::<_, StockMovement>(
        "SELECT id, qty_delta, reason, occurred_at, note FROM stock_movement \
         WHERE item_id = ? ORDER BY id DESC",
    )
    .bind(item_id)
    .fetch_all(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(name: &str) -> ItemInput {
        ItemInput {
            kind: "product".into(),
            name: name.into(),
            sku: name.to_uppercase(),
            unit: "each".into(),
            default_price_minor: 1000,
            default_tax_rate_id: None,
            tracked: true,
            reorder_point: Some(5),
        }
    }

    #[sqlx::test]
    async fn create_and_adjust_stock(pool: Db) -> Result<(), DataError> {
        let id = create(&pool, &product("Widget")).await?;
        assert_eq!(get(&pool, id).await?.unwrap().qty_on_hand, 0);

        adjust_stock(&pool, id, 10, "initial stock").await?;
        adjust_stock(&pool, id, -3, "sold 3").await?;

        let item = get(&pool, id).await?.unwrap();
        assert_eq!(item.qty_on_hand, 7); // cached == ledger sum
        assert_eq!(movements(&pool, id).await?.len(), 2);
        Ok(())
    }

    #[sqlx::test]
    async fn search_and_soft_delete(pool: Db) -> Result<(), DataError> {
        let id = create(&pool, &product("Hammer")).await?;
        create(&pool, &product("Nail")).await?;
        assert_eq!(list(&pool, Some("ham")).await?.len(), 1);
        soft_delete(&pool, id).await?;
        assert!(get(&pool, id).await?.is_none());
        Ok(())
    }
}
