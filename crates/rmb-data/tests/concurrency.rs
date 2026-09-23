//! Adversarial tests for duplicate actions that can arrive before a loading state disables the UI.
//! SQLite is the final authority: each financial transition must remain exactly-once even when two
//! commands start together on separate pooled connections.
//!
//! Each race runs on a database opened exactly as the app opens it (WAL, full sync, busy timeout,
//! pooled connections), with both commands on their own runtime worker threads, and is repeated
//! so a lucky interleaving can't hide a lost update.

use std::future::Future;
use std::path::PathBuf;

use rmb_data::db::{self, Db};
use rmb_data::repos::{customers, invoices, items, jobs, payments, quotes, recurring, settings};
use rmb_data::DataError;

const ROUNDS: usize = 10;

/// A production-configured database in a temporary file.
struct TestDb {
    pool: Db,
    path: PathBuf,
}

impl TestDb {
    async fn open(label: &str) -> Result<Self, DataError> {
        let path = std::env::temp_dir().join(format!(
            "rmb-race-{label}-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        let pool = db::open(&path).await?;
        Ok(Self { pool, path })
    }

    async fn close(self) {
        self.pool.close().await;
        for suffix in ["", "-wal", "-shm"] {
            let mut file = self.path.clone().into_os_string();
            file.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(file));
        }
    }
}

/// Start both commands at once on separate tasks and wait for both.
async fn race<A, B>(a: A, b: B) -> (A::Output, B::Output)
where
    A: Future + Send + 'static,
    B: Future + Send + 'static,
    A::Output: Send + 'static,
    B::Output: Send + 'static,
{
    let (a, b) = tokio::join!(tokio::spawn(a), tokio::spawn(b));
    (
        a.expect("first task panicked"),
        b.expect("second task panicked"),
    )
}

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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_payments_cannot_overallocate() -> Result<(), DataError> {
    let test = TestDb::open("payments").await?;
    let pool = &test.pool;
    for _ in 0..ROUNDS {
        let invoice_id = issued_invoice(pool, 10_000).await?;
        let (a, b) = (pool.clone(), pool.clone());
        let (first, second) = race(
            async move { payments::record_payment(&a, invoice_id, 7_000, "card", "first").await },
            async move { payments::record_payment(&b, invoice_id, 7_000, "card", "second").await },
        )
        .await;
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);

        let detail = invoices::get_detail(pool, invoice_id).await?.unwrap();
        assert_eq!(detail.amount_paid_minor, 7_000);
        assert_eq!(detail.invoice.status, "part_paid");
        assert_eq!(payments::list_for_invoice(pool, invoice_id).await?.len(), 1);
    }
    test.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_issue_assigns_one_number_once() -> Result<(), DataError> {
    let test = TestDb::open("issue").await?;
    let pool = &test.pool;
    let mut business = settings::get(pool).await?;
    business.business_name = "Issue test business".into();
    settings::update(pool, &business).await?;
    let customer_id = customer(pool, "Issue customer").await?;
    for round in 1..=ROUNDS {
        let invoice_id =
            invoices::create_draft(pool, customer_id, &[line("Service", 10_000)], None, "").await?;
        let (a, b) = (pool.clone(), pool.clone());
        let (first, second) = race(
            async move { invoices::issue(&a, invoice_id).await },
            async move { invoices::issue(&b, invoice_id).await },
        )
        .await;
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);

        let detail = invoices::get_detail(pool, invoice_id).await?.unwrap();
        assert_eq!(detail.invoice.status, "issued");
        assert_eq!(detail.invoice.number, Some(format!("INV-{round:04}")));
        assert_eq!(
            settings::get(pool).await?.invoice_next_seq,
            round as i64 + 1
        );
    }
    test.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn void_and_payment_cannot_both_win() -> Result<(), DataError> {
    let test = TestDb::open("void").await?;
    let pool = &test.pool;
    for _ in 0..ROUNDS {
        let invoice_id = issued_invoice(pool, 10_000).await?;
        let (a, b) = (pool.clone(), pool.clone());
        let (voided, paid) = race(
            async move { invoices::void(&a, invoice_id).await },
            async move { payments::record_payment(&b, invoice_id, 10_000, "card", "race").await },
        )
        .await;
        assert_eq!(usize::from(voided.is_ok()) + usize::from(paid.is_ok()), 1);

        let detail = invoices::get_detail(pool, invoice_id).await?.unwrap();
        if voided.is_ok() {
            assert_eq!(detail.invoice.status, "void");
            assert_eq!(detail.amount_paid_minor, 0);
        } else {
            assert_eq!(detail.invoice.status, "paid");
            assert_eq!(detail.amount_paid_minor, 10_000);
        }
    }
    test.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn accepted_quote_has_only_one_downstream_conversion() -> Result<(), DataError> {
    let test = TestDb::open("quote").await?;
    let pool = &test.pool;
    let customer_id = customer(pool, "Quote customer").await?;
    for _ in 0..ROUNDS {
        let quote_id =
            quotes::create_draft(pool, customer_id, &[line("Quoted work", 25_000)], None, "")
                .await?;
        quotes::set_status(pool, quote_id, "sent").await?;
        quotes::set_status(pool, quote_id, "accepted").await?;

        let (a, b) = (pool.clone(), pool.clone());
        let (invoice, job) = race(
            async move { quotes::convert_to_invoice(&a, quote_id).await },
            async move { jobs::create_from_quote(&b, quote_id).await },
        )
        .await;
        assert_eq!(usize::from(invoice.is_ok()) + usize::from(job.is_ok()), 1);

        let downstream: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM invoice WHERE source_quote_id = ?1) + \
                    (SELECT COUNT(*) FROM job WHERE source_quote_id = ?1)",
        )
        .bind(quote_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(downstream, 1);
        assert_eq!(
            quotes::get_detail(pool, quote_id)
                .await?
                .unwrap()
                .quote
                .status,
            "converted"
        );
    }
    test.close().await;
    Ok(())
}

fn time_entry(description: &str) -> jobs::TimeEntryInput {
    jobs::TimeEntryInput {
        date: "2026-08-09".into(),
        minutes: 60,
        rate_minor: 5_000,
        description: description.into(),
        tax_rate_name: "No Tax".into(),
        tax_rate_bp: 0,
        tax_inclusive: false,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn invoicing_a_job_cannot_leave_a_late_unbilled_entry() -> Result<(), DataError> {
    let test = TestDb::open("job").await?;
    let pool = &test.pool;
    let customer_id = customer(pool, "Job customer").await?;
    for _ in 0..ROUNDS {
        let job_id = jobs::create(
            pool,
            &jobs::JobInput {
                customer_id,
                title: "Concurrent job".into(),
                description: String::new(),
            },
        )
        .await?;
        jobs::add_time(pool, job_id, &time_entry("existing")).await?;

        let (a, b) = (pool.clone(), pool.clone());
        let (invoice, add) = race(
            async move { jobs::invoice_from_job(&a, job_id).await },
            async move { jobs::add_time(&b, job_id, &time_entry("late")).await },
        )
        .await;
        assert!(invoice.is_ok(), "the job must be invoiced exactly once");

        let detail = jobs::get_detail(pool, job_id).await?.unwrap();
        assert_eq!(detail.job.status, "invoiced");
        assert!(detail.time_entries.iter().all(|entry| entry.invoiced));
        assert_eq!(detail.time_entries.len(), if add.is_ok() { 2 } else { 1 });
    }
    test.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recurring_period_is_claimed_only_once() -> Result<(), DataError> {
    let test = TestDb::open("recurring").await?;
    let pool = &test.pool;
    let customer_id = customer(pool, "Recurring customer").await?;
    for round in 1..=ROUNDS {
        let schedule_id = recurring::create_as_of(
            pool,
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

        let (a, b) = (pool.clone(), pool.clone());
        let (first, second) = race(
            async move { recurring::run_due(&a, "2026-08-09").await },
            async move { recurring::run_due(&b, "2026-08-09").await },
        )
        .await;
        assert_eq!(first?.created.len() + second?.created.len(), 1);

        let draft_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoice")
            .fetch_one(pool)
            .await?;
        assert_eq!(draft_count, round as i64);
        assert_eq!(
            recurring::get_detail(pool, schedule_id)
                .await?
                .unwrap()
                .schedule
                .next_date,
            "2026-09-09"
        );
    }
    test.close().await;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stock_adjustment_and_tracking_change_preserve_invariants() -> Result<(), DataError> {
    let test = TestDb::open("stock").await?;
    let pool = &test.pool;
    for round in 0..ROUNDS {
        let sku = format!("FILTER-{round:02}");
        let item_id = items::create(pool, &tracked_product("Filter", &sku)).await?;
        let mut untracked = tracked_product("Filter", &sku);
        untracked.tracked = false;

        let (a, b) = (pool.clone(), pool.clone());
        let (adjusted, changed) = race(
            async move { items::adjust_stock(&a, item_id, 1, "received").await },
            async move { items::update(&b, item_id, &untracked).await },
        )
        .await;
        assert_eq!(
            usize::from(adjusted.is_ok()) + usize::from(changed.is_ok()),
            1
        );

        let item = items::get(pool, item_id).await?.unwrap();
        assert!(
            (item.tracked && item.qty_on_hand == 1) || (!item.tracked && item.qty_on_hand == 0),
            "an untracked item must never retain stock"
        );
    }
    test.close().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_catalogue_creates_cannot_duplicate_an_active_sku() -> Result<(), DataError> {
    let test = TestDb::open("sku").await?;
    let pool = &test.pool;
    for round in 0..ROUNDS {
        let first = tracked_product("Filter A", &format!("SAME-{round:02}"));
        let second = tracked_product("Filter B", &format!("same-{round:02}"));

        let (a, b) = (pool.clone(), pool.clone());
        let (created_a, created_b) =
            race(async move { items::create(&a, &first).await }, async move {
                items::create(&b, &second).await
            })
            .await;
        assert_eq!(
            usize::from(created_a.is_ok()) + usize::from(created_b.is_ok()),
            1
        );
        let search = format!("same-{round:02}");
        assert_eq!(items::list(pool, Some(&search)).await?.len(), 1);
    }
    test.close().await;
    Ok(())
}
