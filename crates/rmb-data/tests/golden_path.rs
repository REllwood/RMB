//! Golden-path end-to-end test: the complete story a user drives through the UI, chained
//! against one real database. This is the closest automated stand-in for a manual QA pass —
//! every cross-module seam (quote → job → invoice → payment, stock, history, numbering) in one go.

use rmb_data::db::Db;
use rmb_data::repos::{customers, invoices, items, jobs, payments, quotes, settings};

fn line(item_id: Option<i64>, desc: &str, qty: &str, price: i64, bp: i64) -> invoices::LineInput {
    invoices::LineInput {
        item_id,
        description: desc.into(),
        quantity: qty.into(),
        unit_price_minor: price,
        tax_rate_name: if bp > 0 {
            "GST 10%".into()
        } else {
            "No Tax".into()
        },
        tax_rate_bp: bp,
        tax_inclusive: false,
    }
}

#[sqlx::test]
async fn full_business_story(pool: Db) -> Result<(), rmb_data::error::DataError> {
    // ── Setup: business + AU tax preset ─────────────────────────────────────────────
    settings::apply_tax_preset(&pool, "AU").await?;
    let mut s = settings::get(&pool).await?;
    s.business_name = "Demo Lawn Co".into();
    s.currency = "AUD".into();
    settings::update(&pool, &s).await?;

    // ── Customer + catalog (tracked product with stock) ─────────────────────────────
    let jane = customers::create(
        &pool,
        &customers::CustomerInput {
            name: "Jane Citizen".into(),
            email: "jane@example.com".into(),
            phone: "0400 000 000".into(),
            billing_address: "1 Example St".into(),
            notes: String::new(),
        },
    )
    .await?;
    let turf = items::create(
        &pool,
        &items::ItemInput {
            kind: "product".into(),
            name: "Premium Turf Roll".into(),
            sku: "TURF-1".into(),
            unit: "each".into(),
            default_price_minor: 2500,
            default_tax_rate_id: None,
            tracked: true,
            reorder_point: Some(3),
        },
    )
    .await?;
    items::adjust_stock(&pool, turf, 10, "initial stock").await?;
    assert_eq!(items::get(&pool, turf).await?.unwrap().qty_on_hand, 10);

    // ── Quote: catalog item (4 × $25 + GST) + custom delivery line ──────────────────
    let quote = quotes::create_draft(
        &pool,
        jane,
        &[
            line(Some(turf), "Premium Turf Roll", "4", 2500, 1000),
            line(None, "Delivery", "1", 1500, 1000),
        ],
        Some("2026-07-01"),
        "Front lawn returf",
    )
    .await?;
    let qd = quotes::get_detail(&pool, quote).await?.unwrap();
    assert_eq!(qd.quote.subtotal_minor, 11500); // 100.00 + 15.00
    assert_eq!(qd.quote.tax_minor, 1150); // 10% GST
    assert_eq!(qd.quote.total_minor, 12650);
    assert!(qd.quote.number.is_some());

    // A draft edit keeps the number and recomputes totals.
    quotes::update_draft(
        &pool,
        quote,
        jane,
        &[
            line(Some(turf), "Premium Turf Roll", "4", 2500, 1000),
            line(None, "Delivery", "1", 2000, 1000), // delivery went up
        ],
        Some("2026-07-01"),
        "Front lawn returf",
    )
    .await?;
    assert_eq!(
        quotes::get_detail(&pool, quote)
            .await?
            .unwrap()
            .quote
            .total_minor,
        13200
    );

    // ── Sent → accepted → becomes a JOB (lines arrive as materials) ─────────────────
    quotes::set_status(&pool, quote, "sent").await?;
    quotes::set_status(&pool, quote, "accepted").await?;
    let job = jobs::create_from_quote(&pool, quote).await?;
    let jd = jobs::get_detail(&pool, job).await?.unwrap();
    assert_eq!(jd.job.source_quote_id, Some(quote));
    assert_eq!(jd.materials.len(), 2);
    assert_eq!(jd.materials_total_minor, 12000); // 4×25 + 20
    assert_eq!(
        quotes::get_detail(&pool, quote)
            .await?
            .unwrap()
            .quote
            .status,
        "converted"
    );
    // The same quote can't also become an invoice (no double-billing the scope).
    assert!(quotes::convert_to_invoice(&pool, quote).await.is_err());

    // ── Log labour: 90 min @ $80/h + GST ────────────────────────────────────────────
    jobs::add_time(
        &pool,
        job,
        &jobs::TimeEntryInput {
            date: "2026-06-12".into(),
            minutes: 90,
            rate_minor: 8000,
            description: "lay turf".into(),
            tax_rate_name: "GST 10%".into(),
            tax_rate_bp: 1000,
            tax_inclusive: false,
        },
    )
    .await?;
    assert_eq!(
        jobs::get_detail(&pool, job)
            .await?
            .unwrap()
            .labour_total_minor,
        12000 // 1.5h × $80
    );

    // ── Invoice from job: labour + materials, then ISSUE (stock moves once) ─────────
    let invoice = jobs::invoice_from_job(&pool, job).await?;
    let inv = invoices::get_detail(&pool, invoice).await?.unwrap();
    assert_eq!(inv.lines.len(), 3); // labour + 2 materials
    assert_eq!(inv.invoice.subtotal_minor, 24000); // 120 labour + 120 materials
    assert_eq!(inv.invoice.tax_minor, 2400);
    assert_eq!(inv.invoice.total_minor, 26400);
    assert_eq!(inv.invoice.status, "draft");
    assert_eq!(items::get(&pool, turf).await?.unwrap().qty_on_hand, 10); // drafts never move stock

    invoices::issue(&pool, invoice).await?;
    let issued = invoices::get_detail(&pool, invoice).await?.unwrap();
    assert_eq!(issued.invoice.status, "issued");
    assert_eq!(issued.invoice.number.as_deref(), Some("INV-0001"));
    assert!(issued.invoice.issue_date.is_some());
    assert_eq!(items::get(&pool, turf).await?.unwrap().qty_on_hand, 6); // 10 − 4, exactly once

    // Issued = immutable: no edit, no delete, no re-issue.
    assert!(invoices::delete_draft(&pool, invoice).await.is_err());
    assert!(invoices::issue(&pool, invoice).await.is_err());

    // ── Payments: partial → mis-entry removed → paid ────────────────────────────────
    payments::record_payment(&pool, invoice, 10000, "bank transfer", "JC-1").await?;
    assert_eq!(
        invoices::get_detail(&pool, invoice)
            .await?
            .unwrap()
            .invoice
            .status,
        "part_paid"
    );
    // Fat-fingered second payment gets removed; status rewinds.
    let oops = payments::record_payment(&pool, invoice, 16400, "card", "oops").await?;
    assert_eq!(oops, 16400);
    assert_eq!(
        invoices::get_detail(&pool, invoice)
            .await?
            .unwrap()
            .invoice
            .status,
        "paid"
    );
    let plist = payments::list_for_invoice(&pool, invoice).await?;
    let oops_id = plist.iter().find(|p| p.reference == "oops").unwrap().id;
    payments::delete_payment(&pool, oops_id).await?;
    let after = invoices::get_detail(&pool, invoice).await?.unwrap();
    assert_eq!(after.invoice.status, "part_paid");
    assert_eq!(after.amount_paid_minor, 10000);
    // Void is blocked while money is allocated.
    assert!(invoices::void(&pool, invoice).await.is_err());
    // Settle properly.
    payments::record_payment(&pool, invoice, 16400, "bank transfer", "JC-2").await?;
    assert_eq!(
        invoices::get_detail(&pool, invoice)
            .await?
            .unwrap()
            .invoice
            .status,
        "paid"
    );

    // ── Customer history: everything shows up in one place ──────────────────────────
    assert_eq!(quotes::list_for_customer(&pool, jane).await?.len(), 1);
    assert_eq!(jobs::list_for_customer(&pool, jane).await?.len(), 1);
    assert_eq!(invoices::list_for_customer(&pool, jane).await?.len(), 1);

    // ── A second standalone draft can be edited then deleted without trace ──────────
    let scratch = invoices::create_draft(
        &pool,
        jane,
        &[line(None, "Consult", "1", 5000, 0)],
        None,
        "",
    )
    .await?;
    invoices::update_draft(
        &pool,
        scratch,
        jane,
        &[line(None, "Consult", "2", 5000, 0)],
        None,
        "",
    )
    .await?;
    invoices::delete_draft(&pool, scratch).await?;
    assert!(invoices::get_detail(&pool, scratch).await?.is_none());
    // Gapless numbering unaffected by the deleted draft: next issue is INV-0002.
    let next = invoices::create_draft(
        &pool,
        jane,
        &[line(None, "Tidy-up", "1", 3000, 1000)],
        None,
        "",
    )
    .await?;
    invoices::issue(&pool, next).await?;
    assert_eq!(
        invoices::get_detail(&pool, next)
            .await?
            .unwrap()
            .invoice
            .number
            .as_deref(),
        Some("INV-0002")
    );

    Ok(())
}
