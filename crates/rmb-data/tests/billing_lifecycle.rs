//! Deleting or voiding an invoice must hand its source work back, and voiding must reverse exactly
//! the stock that issuing took — regardless of how the catalogue has changed since.

use rmb_data::db::Db;
use rmb_data::repos::invoices::{self, LineInput};
use rmb_data::repos::{customers, items, jobs, quotes, settings};
use rmb_data::DataError;

async fn customer(db: &Db, name: &str) -> i64 {
    let mut business = settings::get(db).await.unwrap();
    business.business_name = "Test business".into();
    settings::update(db, &business).await.unwrap();
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
    .unwrap()
}

fn line(item_id: Option<i64>, qty: &str, price: i64) -> LineInput {
    LineInput {
        item_id,
        description: "Work".into(),
        quantity: qty.into(),
        unit_price_minor: price,
        tax_rate_name: "No Tax".into(),
        tax_rate_bp: 0,
        tax_inclusive: false,
    }
}

async fn job_with_time(db: &Db, customer_id: i64) -> i64 {
    let job = jobs::create(
        db,
        &jobs::JobInput {
            customer_id,
            title: "Rewire".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap();
    jobs::add_time(
        db,
        job,
        &jobs::TimeEntryInput {
            date: "2026-06-01".into(),
            minutes: 60,
            rate_minor: 5000,
            description: "Labour".into(),
            tax_rate_name: "No Tax".into(),
            tax_rate_bp: 0,
            tax_inclusive: false,
        },
    )
    .await
    .unwrap();
    job
}

async fn accepted_quote(db: &Db, customer_id: i64) -> i64 {
    let quote = quotes::create_draft(db, customer_id, &[line(None, "1", 9000)], None, "")
        .await
        .unwrap();
    quotes::set_status(db, quote, "accepted").await.unwrap();
    quote
}

async fn job_status(db: &Db, job: i64) -> (String, Vec<bool>) {
    let detail = jobs::get_detail(db, job).await.unwrap().unwrap();
    (
        detail.job.status,
        detail.time_entries.iter().map(|t| t.invoiced).collect(),
    )
}

#[sqlx::test]
async fn deleting_a_job_draft_makes_the_work_billable_again(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;
    let job = job_with_time(&pool, c).await;
    let draft = jobs::invoice_from_job(&pool, job).await?;
    assert_eq!(
        job_status(&pool, job).await,
        ("invoiced".into(), vec![true])
    );

    invoices::delete_draft(&pool, draft).await?;
    assert_eq!(job_status(&pool, job).await, ("done".into(), vec![false]));

    // More time can be added before billing again, and the job bills exactly once more.
    jobs::set_status(&pool, job, "in_progress").await?;
    let again = jobs::invoice_from_job(&pool, job).await?;
    assert_ne!(again, draft);
    assert!(jobs::invoice_from_job(&pool, job).await.is_err());
    Ok(())
}

#[sqlx::test]
async fn deleting_a_quote_draft_returns_the_quote_to_accepted(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;
    let quote = accepted_quote(&pool, c).await;
    let draft = quotes::convert_to_invoice(&pool, quote).await?;

    invoices::delete_draft(&pool, draft).await?;
    let detail = quotes::get_detail(&pool, quote).await?.unwrap();
    assert_eq!(detail.quote.status, "accepted");
    assert_eq!(detail.quote.converted_invoice_id, None);
    quotes::convert_to_invoice(&pool, quote).await?;
    Ok(())
}

#[sqlx::test]
async fn voiding_releases_the_source_so_void_and_reissue_works(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;

    let job = job_with_time(&pool, c).await;
    let job_invoice = jobs::invoice_from_job(&pool, job).await?;
    invoices::issue(&pool, job_invoice).await?;
    invoices::void(&pool, job_invoice).await?;
    assert_eq!(job_status(&pool, job).await, ("done".into(), vec![false]));
    let reissued = jobs::invoice_from_job(&pool, job).await?;
    invoices::issue(&pool, reissued).await?;

    let quote = accepted_quote(&pool, c).await;
    let quote_invoice = quotes::convert_to_invoice(&pool, quote).await?;
    invoices::issue(&pool, quote_invoice).await?;
    invoices::void(&pool, quote_invoice).await?;
    assert_eq!(
        quotes::get_detail(&pool, quote)
            .await?
            .unwrap()
            .quote
            .status,
        "accepted"
    );
    quotes::convert_to_invoice(&pool, quote).await?;
    Ok(())
}

#[sqlx::test]
async fn void_reverses_recorded_stock_not_current_settings(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;
    let mut input = items::ItemInput {
        kind: "service".into(),
        name: "Cable".into(),
        sku: String::new(),
        unit: "m".into(),
        default_price_minor: 100,
        default_tax_rate_id: None,
        tracked: false,
        reorder_point: None,
    };
    let item = items::create(&pool, &input).await?;

    // Sold as an untracked service, fractional quantity; then turned into a tracked product.
    let sold = invoices::create_draft(&pool, c, &[line(Some(item), "2.5", 100)], None, "").await?;
    invoices::issue(&pool, sold).await?;
    input.kind = "product".into();
    input.tracked = true;
    items::update(&pool, item, &input).await?;

    invoices::void(&pool, sold).await?;
    assert_eq!(
        items::get(&pool, item).await?.unwrap().qty_on_hand,
        0,
        "stock that was never taken is not put back"
    );
    assert!(items::movements(&pool, item).await?.is_empty());

    // A tracked sale is reversed exactly.
    items::adjust_stock(&pool, item, 10, "opening").await?;
    let tracked_sale =
        invoices::create_draft(&pool, c, &[line(Some(item), "3", 100)], None, "").await?;
    invoices::issue(&pool, tracked_sale).await?;
    assert_eq!(items::get(&pool, item).await?.unwrap().qty_on_hand, 7);
    invoices::void(&pool, tracked_sale).await?;
    assert_eq!(items::get(&pool, item).await?.unwrap().qty_on_hand, 10);
    Ok(())
}

#[sqlx::test]
async fn a_sourced_draft_keeps_its_customer(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;
    let other = customer(&pool, "Globex").await;
    let job = job_with_time(&pool, c).await;
    let draft = jobs::invoice_from_job(&pool, job).await?;

    assert!(
        invoices::update_draft(&pool, draft, other, &[line(None, "1", 100)], None, "")
            .await
            .is_err()
    );
    invoices::update_draft(&pool, draft, c, &[line(None, "1", 100)], None, "edited").await?;
    let detail = invoices::get_detail(&pool, draft).await?.unwrap();
    assert_eq!(detail.invoice.customer_name, "Acme");
    assert_eq!(detail.invoice.source_job_id, Some(job));
    Ok(())
}

#[sqlx::test]
async fn deleting_a_quote_job_returns_the_quote(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;
    let quote = accepted_quote(&pool, c).await;
    let job = jobs::create_from_quote(&pool, quote).await?;
    let material = jobs::get_detail(&pool, job).await?.unwrap().materials[0].id;
    jobs::delete_material(&pool, material).await?;
    jobs::delete(&pool, job).await?;

    assert_eq!(
        quotes::get_detail(&pool, quote)
            .await?
            .unwrap()
            .quote
            .status,
        "accepted"
    );
    jobs::create_from_quote(&pool, quote).await?;
    Ok(())
}

#[sqlx::test]
async fn converted_is_never_set_by_hand(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;
    let quote = accepted_quote(&pool, c).await;
    assert!(quotes::set_status(&pool, quote, "converted").await.is_err());
    assert_eq!(
        quotes::get_detail(&pool, quote)
            .await?
            .unwrap()
            .quote
            .status,
        "accepted"
    );
    Ok(())
}

#[sqlx::test]
async fn extreme_or_long_time_entries_fail_cleanly(pool: Db) -> Result<(), DataError> {
    let c = customer(&pool, "Acme").await;
    let job = job_with_time(&pool, c).await;
    let entry = |minutes: i64, rate_minor: i64, description: String| jobs::TimeEntryInput {
        date: "2026-06-02".into(),
        minutes,
        rate_minor,
        description,
        tax_rate_name: "No Tax".into(),
        tax_rate_bp: 0,
        tax_inclusive: false,
    };
    assert!(
        jobs::add_time(&pool, job, &entry(i64::MAX, i64::MAX, String::new()))
            .await
            .is_err()
    );
    assert!(
        jobs::add_time(&pool, job, &entry(1_000_000, i64::MAX / 2, String::new()))
            .await
            .is_err()
    );

    // A description near the limit still bills: the generated line is shortened, not rejected.
    jobs::add_time(&pool, job, &entry(30, 5000, "x".repeat(1_990))).await?;
    let draft = jobs::invoice_from_job(&pool, job).await?;
    let lines = invoices::get_detail(&pool, draft).await?.unwrap().lines;
    assert!(lines.iter().all(|l| l.description.chars().count() <= 2_000));
    Ok(())
}
