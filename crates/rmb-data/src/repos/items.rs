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

async fn normalise(
    db: &Db,
    input: &ItemInput,
    exclude_id: Option<i64>,
) -> Result<ItemInput, DataError> {
    let value = ItemInput {
        kind: input.kind.trim().to_ascii_lowercase(),
        name: input.name.trim().to_owned(),
        sku: input.sku.trim().to_owned(),
        unit: input.unit.trim().to_owned(),
        default_price_minor: input.default_price_minor,
        default_tax_rate_id: input.default_tax_rate_id,
        tracked: input.tracked,
        reorder_point: input.reorder_point,
    };
    if !matches!(value.kind.as_str(), "product" | "service") {
        return Err(DataError::Other(
            "item type must be product or service".into(),
        ));
    }
    if value.name.is_empty() {
        return Err(DataError::Other("item name is required".into()));
    }
    if value.unit.is_empty() {
        return Err(DataError::Other("item unit is required".into()));
    }
    if value.default_price_minor < 0 {
        return Err(DataError::Other("default price cannot be negative".into()));
    }
    if value.reorder_point.is_some_and(|point| point < 0) {
        return Err(DataError::Other("reorder point cannot be negative".into()));
    }
    if value.kind == "service" && value.tracked {
        return Err(DataError::Other("services cannot track stock".into()));
    }
    if !value.tracked && value.reorder_point.is_some() {
        return Err(DataError::Other(
            "only tracked products can have a reorder point".into(),
        ));
    }
    for (label, text, maximum) in [
        ("item name", value.name.as_str(), 200),
        ("SKU", value.sku.as_str(), 100),
        ("unit", value.unit.as_str(), 50),
    ] {
        if text.chars().count() > maximum {
            return Err(DataError::Other(format!(
                "{label} cannot exceed {maximum} characters"
            )));
        }
    }
    if let Some(tax_id) = value.default_tax_rate_id {
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tax_rate WHERE id = ? AND archived = 0)",
        )
        .bind(tax_id)
        .fetch_one(db)
        .await?;
        if !active {
            return Err(DataError::Other("default tax rate was not found".into()));
        }
    }
    if !value.sku.is_empty() {
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM item WHERE deleted_at IS NULL \
             AND lower(sku) = lower(?) AND id != ?)",
        )
        .bind(&value.sku)
        .bind(exclude_id.unwrap_or(-1))
        .fetch_one(db)
        .await?;
        if duplicate {
            return Err(DataError::Other("SKU is already in use".into()));
        }
    }
    Ok(value)
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
    let input = normalise(db, input, None).await?;
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
    let input = normalise(db, input, Some(id)).await?;
    // qty_on_hand is never set directly — only via stock movements.
    let result = sqlx::query(
        "UPDATE item SET kind=?, name=?, sku=?, unit=?, default_price_minor=?, default_tax_rate_id=?, \
         tracked=?, reorder_point=? WHERE id=? AND deleted_at IS NULL \
         AND (? = 1 OR (qty_on_hand = 0 AND NOT EXISTS ( \
             SELECT 1 FROM stock_movement WHERE item_id = item.id)))",
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
    .bind(input.tracked)
    .execute(db)
    .await?;
    if result.rows_affected() != 1 {
        return Err(DataError::Other(
            "item not found, or tracking cannot be disabled after stock has been recorded; archive the item and create a new one instead"
                .into(),
        ));
    }
    Ok(())
}

pub async fn soft_delete(db: &Db, id: i64) -> Result<(), DataError> {
    let result = sqlx::query(
        "UPDATE item SET deleted_at = datetime('now') \
         WHERE id = ?1 AND deleted_at IS NULL AND (tracked = 0 OR qty_on_hand = 0) \
           AND NOT EXISTS (SELECT 1 FROM invoice_line l JOIN invoice i ON i.id = l.invoice_id \
                           WHERE l.item_id = ?1 AND i.status = 'draft') \
           AND NOT EXISTS (SELECT 1 FROM quote_line l JOIN quote q ON q.id = l.quote_id \
                           WHERE l.item_id = ?1 AND q.deleted_at IS NULL \
                             AND q.status NOT IN ('declined', 'expired', 'converted')) \
           AND NOT EXISTS (SELECT 1 FROM job_material m JOIN job j ON j.id = m.job_id \
                           WHERE m.item_id = ?1 AND m.invoiced = 0 AND j.deleted_at IS NULL) \
           AND NOT EXISTS (SELECT 1 FROM recurring_invoice_line l \
                           JOIN recurring_invoice r ON r.id = l.recurring_id \
                           WHERE l.item_id = ?1 AND r.active = 1)",
    )
    .bind(id)
    .execute(db)
    .await?;
    if result.rows_affected() != 1 {
        return Err(DataError::Other(
            "item not found, has stock on hand, or is used by draft or active work".into(),
        ));
    }
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
    if qty_delta == 0 {
        return Err(DataError::Other(
            "stock movement must change the quantity".into(),
        ));
    }
    if note.chars().count() > 2_000 {
        return Err(DataError::Other(
            "stock movement note cannot exceed 2,000 characters".into(),
        ));
    }
    let item: Option<(String, bool, i64)> =
        sqlx::query_as("SELECT kind, tracked, qty_on_hand FROM item WHERE id = ?")
            .bind(item_id)
            .fetch_optional(&mut *conn)
            .await?;
    let (kind, tracked, current_qty) =
        item.ok_or_else(|| DataError::Other("item not found".into()))?;
    if kind != "product" || !tracked {
        return Err(DataError::Other(
            "stock can only be adjusted for tracked products".into(),
        ));
    }
    let new_qty = current_qty
        .checked_add(qty_delta)
        .ok_or_else(|| DataError::Other("stock quantity is too large".into()))?;

    sqlx::query(
        "INSERT INTO stock_movement (item_id, qty_delta, reason, ref_type, ref_id, note) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(item_id)
    .bind(qty_delta)
    .bind(reason.as_db())
    .bind(ref_type)
    .bind(ref_id)
    .bind(note.trim())
    .execute(&mut *conn)
    .await?;
    let result = sqlx::query("UPDATE item SET qty_on_hand = ? WHERE id = ?")
        .bind(new_qty)
        .bind(item_id)
        .execute(&mut *conn)
        .await?;
    if result.rows_affected() != 1 {
        return Err(DataError::Other("item not found".into()));
    }
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
    let claimed: Option<i64> = sqlx::query_scalar(
        "UPDATE item SET qty_on_hand = qty_on_hand \
         WHERE id = ? AND deleted_at IS NULL AND kind = 'product' AND tracked = 1 RETURNING id",
    )
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await?;
    if claimed.is_none() {
        return Err(DataError::Other(
            "stock can only be adjusted for an active tracked product".into(),
        ));
    }
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

    #[sqlx::test]
    async fn rejects_invalid_items_and_stock_movements(pool: Db) -> Result<(), DataError> {
        let mut invalid = product("Widget");
        invalid.kind = "service".into();
        assert!(create(&pool, &invalid).await.is_err());

        invalid.tracked = false;
        invalid.reorder_point = None;
        let service = create(&pool, &invalid).await?;
        assert!(adjust_stock(&pool, service, 1, "not stock").await.is_err());
        assert!(adjust_stock(&pool, 99_999, 1, "missing").await.is_err());

        let product_id = create(&pool, &product("Product")).await?;
        assert!(adjust_stock(&pool, product_id, 0, "no change")
            .await
            .is_err());
        assert!(update(&pool, 99_999, &product("Missing")).await.is_err());
        assert!(soft_delete(&pool, 99_999).await.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn stock_must_be_zero_before_tracking_is_disabled_or_item_deleted(
        pool: Db,
    ) -> Result<(), DataError> {
        let id = create(&pool, &product("Widget")).await?;
        adjust_stock(&pool, id, 2, "opening stock").await?;

        let mut untracked = product("Widget");
        untracked.tracked = false;
        untracked.reorder_point = None;
        assert!(update(&pool, id, &untracked).await.is_err());
        assert!(soft_delete(&pool, id).await.is_err());

        adjust_stock(&pool, id, -2, "write off").await?;
        assert!(
            update(&pool, id, &untracked).await.is_err(),
            "stock history keeps the item's tracking semantics immutable"
        );
        soft_delete(&pool, id).await?;
        assert!(adjust_stock(&pool, id, 1, "deleted").await.is_err());
        Ok(())
    }
}
