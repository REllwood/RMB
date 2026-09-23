//! Quote (estimate) repository. Mirrors invoices (no stock/payments). A quote can convert into a
//! draft invoice, copying its lines and linking both directions.

use rmb_domain::numbering::format_number;
use rmb_domain::status::QuoteStatus;
use rmb_domain::tax::line_tax;
use serde::Serialize;
use sqlx::FromRow;

use crate::db::{begin_write, Db};
use crate::error::DataError;
use crate::repos::invoices::{
    self, to_doc_line, validate_draft_metadata, validate_line_items, validated_totals, LineInput,
};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct QuoteRow {
    pub id: i64,
    pub customer_id: i64,
    pub number: Option<String>,
    pub status: String,
    pub valid_until: Option<String>,
    pub subtotal_minor: i64,
    pub tax_minor: i64,
    pub total_minor: i64,
    pub notes: String,
    pub converted_invoice_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct QuoteLineRow {
    pub id: i64,
    pub item_id: Option<i64>,
    pub description: String,
    pub quantity: String,
    pub unit_price_minor: i64,
    pub tax_rate_name: String,
    pub tax_rate_bp: i64,
    pub tax_inclusive: bool,
    pub net_minor: i64,
    pub tax_minor: i64,
    pub gross_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuoteDetail {
    pub quote: QuoteRow,
    pub lines: Vec<QuoteLineRow>,
    pub tax_summary: String,
}

pub async fn create_draft(
    db: &Db,
    customer_id: i64,
    lines: &[LineInput],
    valid_until: Option<&str>,
    notes: &str,
) -> Result<i64, DataError> {
    if lines.is_empty() {
        return Err(DataError::Other("a quote needs at least one line".into()));
    }
    let totals = validated_totals(lines)?;
    let tax_summary = serde_json::to_string(&totals.tax_summary).unwrap_or_else(|_| "[]".into());

    let mut tx = begin_write(db).await?;
    validate_draft_metadata(&mut tx, customer_id, valid_until, "valid-until date", notes).await?;
    validate_line_items(&mut tx, lines).await?;
    let valid_until = valid_until.map(str::trim);
    // Quotes are estimates: a number is assigned now from the quote counter. Unlike invoices, quote
    // numbering is intentionally **not** gapless — deleting/abandoning a draft can leave a gap.
    // (Legally-gapless invoice numbers are assigned only at issue.)
    let (prefix, seq, pad) = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT quote_prefix, quote_next_seq, number_pad FROM settings WHERE id = 1",
    )
    .fetch_one(&mut *tx)
    .await?;
    let number = format_number(&prefix, seq, pad.max(0) as usize);
    sqlx::query("UPDATE settings SET quote_next_seq = quote_next_seq + 1 WHERE id = 1")
        .execute(&mut *tx)
        .await?;

    let quote_id = sqlx::query(
        "INSERT INTO quote (customer_id, number, status, valid_until, subtotal_minor, tax_minor, total_minor, tax_summary, notes) \
         VALUES (?, ?, 'draft', ?, ?, ?, ?, ?, ?)",
    )
    .bind(customer_id)
    .bind(&number)
    .bind(valid_until)
    .bind(totals.subtotal.minor())
    .bind(totals.tax_total.minor())
    .bind(totals.total.minor())
    .bind(tax_summary)
    .bind(notes)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();

    insert_lines(&mut tx, quote_id, lines).await?;
    tx.commit().await?;
    Ok(quote_id)
}

/// Insert the computed line rows for a quote (shared by create + update-draft).
async fn insert_lines(
    conn: &mut sqlx::SqliteConnection,
    quote_id: i64,
    lines: &[LineInput],
) -> Result<(), DataError> {
    for (i, l) in lines.iter().enumerate() {
        let dl = to_doc_line(l)?;
        let lt = line_tax(dl.line_amount(), &dl.tax_rate);
        sqlx::query(
            "INSERT INTO quote_line (quote_id, item_id, description, quantity, unit_price_minor, \
             tax_rate_name, tax_rate_bp, tax_inclusive, net_minor, tax_minor, gross_minor, line_order) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(quote_id)
        .bind(l.item_id)
        .bind(&dl.description)
        .bind(dl.quantity.normalize().to_string())
        .bind(l.unit_price_minor)
        .bind(&dl.tax_rate.name)
        .bind(l.tax_rate_bp)
        .bind(l.tax_inclusive)
        .bind(lt.net.minor())
        .bind(lt.tax.minor())
        .bind(lt.gross.minor())
        .bind(i as i64)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Replace a **draft** quote's customer, lines, validity, and notes (recomputing totals).
/// A sent quote must be pulled back to draft first (`sent → draft` is a legal transition).
pub async fn update_draft(
    db: &Db,
    id: i64,
    customer_id: i64,
    lines: &[LineInput],
    valid_until: Option<&str>,
    notes: &str,
) -> Result<(), DataError> {
    if lines.is_empty() {
        return Err(DataError::Other("a quote needs at least one line".into()));
    }
    let totals = validated_totals(lines)?;
    let tax_summary = serde_json::to_string(&totals.tax_summary).unwrap_or_else(|_| "[]".into());

    let mut tx = begin_write(db).await?;
    let claimed: Option<i64> = sqlx::query_scalar(
        "UPDATE quote SET status = status \
         WHERE id = ? AND deleted_at IS NULL AND status = 'draft' RETURNING id",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if claimed.is_none() {
        return Err(DataError::Other(
            "only draft quotes can be edited (pull a sent quote back to draft first)".into(),
        ));
    }
    validate_draft_metadata(&mut tx, customer_id, valid_until, "valid-until date", notes).await?;
    validate_line_items(&mut tx, lines).await?;
    let valid_until = valid_until.map(str::trim);

    sqlx::query(
        "UPDATE quote SET customer_id = ?, valid_until = ?, notes = ?, subtotal_minor = ?, \
         tax_minor = ?, total_minor = ?, tax_summary = ? WHERE id = ?",
    )
    .bind(customer_id)
    .bind(valid_until)
    .bind(notes)
    .bind(totals.subtotal.minor())
    .bind(totals.tax_total.minor())
    .bind(totals.total.minor())
    .bind(tax_summary)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM quote_line WHERE quote_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    insert_lines(&mut tx, id, lines).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list(db: &Db) -> Result<Vec<QuoteRow>, DataError> {
    Ok(sqlx::query_as::<_, QuoteRow>(
        "SELECT id, customer_id, number, status, valid_until, subtotal_minor, tax_minor, \
         total_minor, notes, converted_invoice_id, created_at FROM quote \
         WHERE deleted_at IS NULL ORDER BY id DESC",
    )
    .fetch_all(db)
    .await?)
}

pub async fn list_for_customer(db: &Db, customer_id: i64) -> Result<Vec<QuoteRow>, DataError> {
    Ok(sqlx::query_as::<_, QuoteRow>(
        "SELECT id, customer_id, number, status, valid_until, subtotal_minor, tax_minor, \
         total_minor, notes, converted_invoice_id, created_at FROM quote \
         WHERE customer_id = ? AND deleted_at IS NULL ORDER BY id DESC",
    )
    .bind(customer_id)
    .fetch_all(db)
    .await?)
}

pub async fn get_detail(db: &Db, id: i64) -> Result<Option<QuoteDetail>, DataError> {
    let quote = sqlx::query_as::<_, QuoteRow>(
        "SELECT id, customer_id, number, status, valid_until, subtotal_minor, tax_minor, \
         total_minor, notes, converted_invoice_id, created_at FROM quote \
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some(quote) = quote else { return Ok(None) };
    let lines = sqlx::query_as::<_, QuoteLineRow>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive, net_minor, tax_minor, gross_minor FROM quote_line WHERE quote_id = ? ORDER BY line_order",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let tax_summary: String = sqlx::query_scalar("SELECT tax_summary FROM quote WHERE id = ?")
        .bind(id)
        .fetch_one(db)
        .await?;
    Ok(Some(QuoteDetail {
        quote,
        lines,
        tax_summary,
    }))
}

pub async fn set_status(db: &Db, id: i64, to: &str) -> Result<(), DataError> {
    let current: Option<String> =
        sqlx::query_scalar("SELECT status FROM quote WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(db)
            .await?;
    let current = current.ok_or_else(|| DataError::Other("quote not found".into()))?;
    let from = QuoteStatus::from_db(&current)
        .ok_or_else(|| DataError::Other("invalid current status".into()))?;
    let target = QuoteStatus::from_db(to)
        .ok_or_else(|| DataError::Other(format!("unknown status '{to}'")))?;
    if target == QuoteStatus::Converted {
        return Err(DataError::Other(
            "a quote is marked converted automatically when you convert it to an invoice or job"
                .into(),
        ));
    }
    if !from.can_transition_to(target) {
        return Err(DataError::Other(format!(
            "a {current} quote can't be marked {to}"
        )));
    }
    let changed = sqlx::query(
        "UPDATE quote SET status = ? WHERE id = ? AND deleted_at IS NULL AND status = ?",
    )
    .bind(target.as_db())
    .bind(id)
    .bind(&current)
    .execute(db)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(DataError::Other(
            "quote changed while its status was being updated; try again".into(),
        ));
    }
    Ok(())
}

pub async fn delete(db: &Db, id: i64) -> Result<(), DataError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM quote WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(db)
            .await?;
    let status = status.ok_or_else(|| DataError::Other("quote not found".into()))?;
    if !matches!(status.as_str(), "draft" | "declined" | "expired") {
        return Err(DataError::Other(
            "sent, accepted, or converted quotes cannot be deleted; decline an active quote first"
                .into(),
        ));
    }
    let changed = sqlx::query(
        "UPDATE quote SET deleted_at = datetime('now') \
         WHERE id = ? AND deleted_at IS NULL AND status = ?",
    )
    .bind(id)
    .bind(status)
    .execute(db)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(DataError::Other(
            "quote changed while it was being deleted; try again".into(),
        ));
    }
    Ok(())
}

/// Convert an **accepted** quote into a draft invoice (copying its lines), link both, and mark it
/// converted — all in one transaction so a crash can't orphan a draft or permit a second conversion.
pub async fn convert_to_invoice(db: &Db, id: i64) -> Result<i64, DataError> {
    // Claim first. This is the transaction's first database operation, so competing invoice/job
    // conversions cannot both read "accepted" and bill the same quote.
    let mut tx = begin_write(db).await?;
    let source: Option<(i64, String)> = sqlx::query_as(
        "UPDATE quote SET status = 'converted' \
         WHERE id = ? AND deleted_at IS NULL AND status = 'accepted' \
         RETURNING customer_id, notes",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (customer_id, notes) = source
        .ok_or_else(|| DataError::Other("quote not found or it is no longer accepted".into()))?;
    let quote_lines = sqlx::query_as::<_, QuoteLineRow>(
        "SELECT id, item_id, description, quantity, unit_price_minor, tax_rate_name, tax_rate_bp, \
         tax_inclusive, net_minor, tax_minor, gross_minor FROM quote_line \
         WHERE quote_id = ? ORDER BY line_order",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let lines: Vec<LineInput> = quote_lines
        .iter()
        .map(|l| LineInput {
            item_id: l.item_id,
            description: l.description.clone(),
            quantity: l.quantity.clone(),
            unit_price_minor: l.unit_price_minor,
            tax_rate_name: l.tax_rate_name.clone(),
            tax_rate_bp: l.tax_rate_bp,
            tax_inclusive: l.tax_inclusive,
        })
        .collect();

    let invoice_id = invoices::create_draft_on(&mut tx, customer_id, &lines, None, &notes).await?;
    sqlx::query("UPDATE quote SET converted_invoice_id = ? WHERE id = ?")
        .bind(invoice_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE invoice SET source_quote_id = ? WHERE id = ?")
        .bind(id)
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(invoice_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::customers;

    #[sqlx::test]
    async fn create_status_transitions_and_convert(pool: Db) -> Result<(), DataError> {
        let customer = customers::create(
            &pool,
            &customers::CustomerInput {
                name: "Acme".into(),
                email: String::new(),
                phone: String::new(),
                billing_address: String::new(),
                notes: String::new(),
            },
        )
        .await?;
        let line = LineInput {
            item_id: None,
            description: "Design work".into(),
            quantity: "10".into(),
            unit_price_minor: 5000,
            tax_rate_name: "VAT 20%".into(),
            tax_rate_bp: 2000,
            tax_inclusive: false,
        };
        let q = create_draft(&pool, customer, &[line], Some("2026-07-01"), "thanks").await?;

        let d = get_detail(&pool, q).await?.unwrap();
        assert_eq!(d.quote.status, "draft");
        assert!(d.quote.number.is_some());
        assert_eq!(d.quote.subtotal_minor, 50000);
        assert_eq!(d.quote.tax_minor, 10000);
        assert_eq!(d.quote.total_minor, 60000);

        // Draft edits recompute totals; the assigned number is kept.
        let edited = LineInput {
            item_id: None,
            description: "Design work (revised)".into(),
            quantity: "2".into(),
            unit_price_minor: 5000,
            tax_rate_name: "VAT 20%".into(),
            tax_rate_bp: 2000,
            tax_inclusive: false,
        };
        let number_before = d.quote.number.clone();
        update_draft(&pool, q, customer, &[edited], None, "revised").await?;
        let d2 = get_detail(&pool, q).await?.unwrap();
        assert_eq!(d2.quote.total_minor, 12000); // 2 × $50 + 20%
        assert_eq!(d2.quote.number, number_before);
        assert_eq!(d2.lines.len(), 1);

        set_status(&pool, q, "sent").await?;
        // Sent quotes are not editable (pull back to draft first).
        assert!(update_draft(&pool, q, customer, &[], None, "")
            .await
            .is_err());
        set_status(&pool, q, "accepted").await?;
        assert!(set_status(&pool, q, "draft").await.is_err()); // illegal accepted→draft

        let invoice_id = convert_to_invoice(&pool, q).await?;
        assert_eq!(
            get_detail(&pool, q).await?.unwrap().quote.status,
            "converted"
        );
        let inv_detail = invoices::get_detail(&pool, invoice_id).await?.unwrap();
        assert_eq!(inv_detail.invoice.total_minor, 12000); // matches the edited quote
        Ok(())
    }
}
