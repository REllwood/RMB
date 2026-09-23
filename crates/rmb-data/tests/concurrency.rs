//! Adversarial tests for duplicate actions that can arrive before a loading state disables the UI.
//! SQLite is the final authority: each financial transition must remain exactly-once even when two
//! commands start together on separate pooled connections.

use rmb_data::db::Db;
use rmb_data::repos::{customers, invoices, items, jobs, payments, quotes, recurring, settings};
use rmb_data::DataError;

async fn customer(db: &Db, name: &str) -> Result<i64, DataError> {
    customers::create(
        db,
        &customers::CustomerInput {
            name: name.into(),
            email: String::new(),
            phone: String::new(),
            billing_address: String::new(),
            notes: String::new(),
        },
    )
    .await
}

fn line(description: &str, price: i64) -> invoices::LineInput {
    invoices::LineInput {
        item_id: None,
        description: description.into(),
        quantity: "1".into(),
        unit_price_minor: price,
        tax_rate_name: "No Tax".into(),
        tax_rate_bp: 0,
        tax_inclusive: false,
    }
}

async fn issued_invoice(db: &Db, amount: i64) -> Result<i64, DataError> {
    let mut business = settings::get(db).await?;
    business.business_name = "Concurrency test business".into();
    settings::update(db, &business).await?;
    let customer_id = customer(db, "Payment customer").await?;
    let invoice_id =
        invoices::create_draft(db, customer_id, &[line("Service", amount)], None, "").await?;
    invoices::issue(db, invoice_id).await?;
    Ok(invoice_id)
}

#[sqlx::test]
async fn simultaneous_payments_cannot_overallocate(pool: Db) -> Result<(), DataError> {
    let invoice_id = issued_invoice(&pool, 10_000).await?;

    let (first, second) = tokio::join!(
        payments::record_payment(&pool, invoice_id, 7_000, "card", "first"),
        payments::record_payment(&pool, invoice_id, 7_000, "card", "second"),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);

    let detail = invoices::get_detail(&pool, invoice_id).await?.unwrap();
    assert_eq!(detail.amount_paid_minor, 7_000);
    assert_eq!(detail.invoice.status, "part_paid");
    assert_eq!(
        payments::list_for_invoice(&pool, invoice_id).await?.len(),
        1
    );
    Ok(())
}

#[sqlx::test]
async fn simultaneous_issue_assigns_one_number_once(pool: Db) -> Result<(), DataError> {
    let mut business = settings::get(&pool).await?;
    business.business_name = "Issue test business".into();
    settings::update(&pool, &business).await?;
    let customer_id = customer(&pool, "Issue customer").await?;
    let invoice_id =
        invoices::create_draft(&pool, customer_id, &[line("Service", 10_000)], None, "").await?;

    let (first, second) = tokio::join!(
        invoices::issue(&pool, invoice_id),
        invoices::issue(&pool, invoice_id),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);

    let detail = invoices::get_detail(&pool, invoice_id).await?.unwrap();
    assert_eq!(detail.invoice.status, "issued");
    assert_eq!(detail.invoice.number.as_deref(), Some("INV-0001"));
    assert_eq!(settings::get(&pool).await?.invoice_next_seq, 2);
    Ok(())
}

#[sqlx::test]
async fn void_and_payment_cannot_both_win(pool: Db) -> Result<(), DataError> {
    let invoice_id = issued_invoice(&pool, 10_000).await?;

    let (voided, paid) = tokio::join!(
        invoices::void(&pool, invoice_id),
        payments::record_payment(&pool, invoice_id, 10_000, "card", "race"),
    );
    assert_eq!(usize::from(voided.is_ok()) + usize::from(paid.is_ok()), 1);

    let detail = invoices::get_detail(&pool, invoice_id).await?.unwrap();
    if voided.is_ok() {
        assert_eq!(detail.invoice.status, "void");
        assert_eq!(detail.amount_paid_minor, 0);
    } else {
        assert_eq!(detail.invoice.status, "paid");
        assert_eq!(detail.amount_paid_minor, 10_000);
    }
    Ok(())
}

#[sqlx::test]
async fn accepted_quote_has_only_one_downstream_conversion(pool: Db) -> Result<(), DataError> {
    let customer_id = customer(&pool, "Quote customer").await?;
    let quote_id =
        quotes::create_draft(&pool, customer_id, &[line("Quoted work", 25_000)], None, "").await?;
    quotes::set_status(&pool, quote_id, "sent").await?;
    quotes::set_status(&pool, quote_id, "accepted").await?;

    let (invoice, job) = tokio::join!(
        quotes::convert_to_invoice(&pool, quote_id),
        jobs::create_from_quote(&pool, quote_id),
    );
    assert_eq!(usize::from(invoice.is_ok()) + usize::from(job.is_ok()), 1);

    let downstream: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM invoice WHERE source_quote_id = ?1) + \
                (SELECT COUNT(*) FROM job WHERE source_quote_id = ?1)",
    )
    .bind(quote_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(downstream, 1);
    assert_eq!(
        quotes::get_detail(&pool, quote_id)
            .await?
            .unwrap()
            .quote
            .status,
        "converted"
    );
    Ok(())
}

#[sqlx::test]
async fn invoicing_a_job_cannot_leave_a_late_unbilled_entry(pool: Db) -> Result<(), DataError> {
    let customer_id = customer(&pool, "Job customer").await?;
    let job_id = jobs::create(
        &pool,
        &jobs::JobInput {
            customer_id,
            title: "Concurrent job".into(),
            description: String::new(),
        },
    )
    .await?;
    let entry = |description: &str| jobs::TimeEntryInput {
        date: "2026-08-09".into(),
        minutes: 60,
        rate_minor: 5_000,
        description: description.into(),
        tax_rate_name: "No Tax".into(),
        tax_rate_bp: 0,
        tax_inclusive: false,
    };
    jobs::add_time(&pool, job_id, &entry("existing")).await?;
    let late = entry("late");

    let (invoice, add) = tokio::join!(
        jobs::invoice_from_job(&pool, job_id),
        jobs::add_time(&pool, job_id, &late),
    );
    assert!(invoice.is_ok(), "the job must be invoiced exactly once");

    let detail = jobs::get_detail(&pool, job_id).await?.unwrap();
    assert_eq!(detail.job.status, "invoiced");
    assert!(detail.time_entries.iter().all(|entry| entry.invoiced));
    assert_eq!(detail.time_entries.len(), if add.is_ok() { 2 } else { 1 });
    Ok(())
}

#[sqlx::test]
async fn recurring_period_is_claimed_only_once(pool: Db) -> Result<(), DataError> {
    let customer_id = customer(&pool, "Recurring customer").await?;
    let schedule_id = recurring::create_as_of(
        &pool,
        &recurring::RecurringInput {
            customer_id,
            frequency: "monthly".into(),
            next_date: "2026-08-09".into(),
            end_date: None,
            due_days: None,
            notes: String::new(),
        },
        &[line("Monthly service", 10_000)],
        "2026-08-09",
    )
    .await?;

    let (first, second) = tokio::join!(
        recurring::run_due(&pool, "2026-08-09"),
        recurring::run_due(&pool, "2026-08-09"),
    );
    let generated = first?.created.len() + second?.created.len();
    assert_eq!(generated, 1);

    let draft_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice")
        .fetch_one(&pool)
        .await?;
    assert_eq!(draft_count, 1);
    assert_eq!(
        recurring::get_detail(&pool, schedule_id)
            .await?
            .unwrap()
            .schedule
            .next_date,
        "2026-09-09"
    );
    Ok(())
}

fn tracked_product(name: &str, sku: &str) -> items::ItemInput {
    items::ItemInput {
        kind: "product".into(),
        name: name.into(),
        sku: sku.into(),
        unit: "each".into(),
        default_price_minor: 1_000,
        default_tax_rate_id: None,
        tracked: true,
        reorder_point: None,
    }
}

#[sqlx::test]
async fn stock_adjustment_and_tracking_change_preserve_invariants(
    pool: Db,
) -> Result<(), DataError> {
    let item_id = items::create(&pool, &tracked_product("Filter", "FILTER")).await?;
    let mut untracked = tracked_product("Filter", "FILTER");
    untracked.tracked = false;

    let (adjusted, changed) = tokio::join!(
        items::adjust_stock(&pool, item_id, 1, "received"),
        items::update(&pool, item_id, &untracked),
    );
    assert_eq!(
        usize::from(adjusted.is_ok()) + usize::from(changed.is_ok()),
        1
    );

    let item = items::get(&pool, item_id).await?.unwrap();
    assert!(
        (item.tracked && item.qty_on_hand == 1) || (!item.tracked && item.qty_on_hand == 0),
        "an untracked item must never retain stock"
    );
    Ok(())
}

#[sqlx::test]
async fn simultaneous_catalogue_creates_cannot_duplicate_an_active_sku(
    pool: Db,
) -> Result<(), DataError> {
    let first = tracked_product("Filter A", "SAME-SKU");
    let second = tracked_product("Filter B", "same-sku");

    let (a, b) = tokio::join!(items::create(&pool, &first), items::create(&pool, &second));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(items::list(&pool, Some("same-sku")).await?.len(), 1);
    Ok(())
}
