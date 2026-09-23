//! Invariants that must hold after any sequence of user actions: stock equals its movement
//! history, document headers equal their lines, invoice status matches the money received, and
//! every report and export tells the same story. A long, seeded mix of real operations (including
//! ones the app rejects) is applied, and everything is checked after every step.

use rmb_data::db::Db;
use rmb_data::repos::{customers, invoices, items, payments, reports, settings};
use rmb_data::DataError;

/// Small deterministic generator, so a failure always replays the same way.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<'a, T>(&mut self, values: &'a [T]) -> Option<&'a T> {
        if values.is_empty() {
            None
        } else {
            values.get(self.below(values.len() as u64) as usize)
        }
    }
}

const RATES: [(&str, i64, bool); 4] = [
    ("No Tax", 0, false),
    ("VAT 20%", 2_000, false),
    ("GST 10% incl.", 1_000, true),
    ("Sales tax 8.875%", 888, false),
];

async fn ids(db: &Db, sql: &'static str) -> Result<Vec<i64>, DataError> {
    Ok(sqlx::query_scalar(sql).fetch_all(db).await?)
}

async fn random_line(db: &Db, rng: &mut Lcg) -> Result<invoices::LineInput, DataError> {
    let (tax_rate_name, tax_rate_bp, tax_inclusive) = RATES[rng.below(RATES.len() as u64) as usize];
    let products = ids(
        db,
        "SELECT id FROM item WHERE deleted_at IS NULL AND tracked = 1",
    )
    .await?;
    let use_item = rng.below(2) == 0;
    let (item_id, quantity) = match (use_item, rng.pick(&products)) {
        // Tracked products sell in whole units.
        (true, Some(&id)) => (Some(id), (1 + rng.below(4)).to_string()),
        _ => (
            None,
            ["1", "0.5", "2.25", "1.333", "3"][rng.below(5) as usize].to_string(),
        ),
    };
    // Mostly charges, sometimes a discount line.
    let unit_price_minor = if rng.below(8) == 0 {
        -(rng.below(500) as i64)
    } else {
        1 + rng.below(50_000) as i64
    };
    Ok(invoices::LineInput {
        item_id,
        description: format!("Line {}", rng.next() % 1_000),
        quantity,
        unit_price_minor,
        tax_rate_name: tax_rate_name.into(),
        tax_rate_bp,
        tax_inclusive,
    })
}

async fn step(db: &Db, rng: &mut Lcg, customer_ids: &[i64]) -> Result<(), DataError> {
    // Rejected actions are part of the test: whatever fails must leave everything consistent.
    let _ = match rng.below(9) {
        0 => {
            let name = format!("Product {}", rng.next());
            let id = items::create(
                db,
                &items::ItemInput {
                    kind: "product".into(),
                    name: name.clone(),
                    sku: name.to_uppercase().replace(' ', "-"),
                    unit: "each".into(),
                    default_price_minor: 1_000,
                    default_tax_rate_id: None,
                    tracked: true,
                    reorder_point: None,
                },
            )
            .await?;
            items::adjust_stock(db, id, 20, "opening").await
        }
        1 => {
            let products = ids(db, "SELECT id FROM item WHERE tracked = 1").await?;
            match rng.pick(&products) {
                Some(&id) => items::adjust_stock(db, id, rng.below(11) as i64 - 5, "count").await,
                None => Ok(()),
            }
        }
        2 | 3 => {
            let mut lines = Vec::new();
            for _ in 0..1 + rng.below(4) {
                lines.push(random_line(db, rng).await?);
            }
            let customer_id = *rng.pick(customer_ids).expect("customers exist");
            invoices::create_draft(db, customer_id, &lines, None, "")
                .await
                .map(|_| ())
        }
        4 => {
            let drafts = ids(db, "SELECT id FROM invoice WHERE status = 'draft'").await?;
            match rng.pick(&drafts) {
                Some(&id) => invoices::issue(db, id).await,
                None => Ok(()),
            }
        }
        5 | 6 => {
            let open = ids(
                db,
                "SELECT id FROM invoice WHERE status IN ('issued', 'part_paid')",
            )
            .await?;
            match rng.pick(&open) {
                Some(&id) => {
                    let detail = invoices::get_detail(db, id).await?.expect("invoice exists");
                    let balance = detail.invoice.total_minor - detail.amount_paid_minor;
                    // Sometimes the whole balance, sometimes part, sometimes too much.
                    let amount = match rng.below(3) {
                        0 => balance,
                        1 => (balance / 2).max(1),
                        _ => balance + 1,
                    };
                    payments::record_payment(db, id, amount, "card", "")
                        .await
                        .map(|_| ())
                }
                None => Ok(()),
            }
        }
        7 => {
            let paid = ids(db, "SELECT id FROM payment").await?;
            match rng.pick(&paid) {
                Some(&id) => payments::delete_payment(db, id).await,
                None => Ok(()),
            }
        }
        _ => {
            let issued = ids(
                db,
                "SELECT id FROM invoice WHERE status IN ('issued', 'part_paid', 'paid')",
            )
            .await?;
            match rng.pick(&issued) {
                Some(&id) => invoices::void(db, id).await,
                None => Ok(()),
            }
        }
    };
    Ok(())
}

async fn check(db: &Db, context: &str) -> Result<(), DataError> {
    // Stock on hand is exactly the sum of its recorded movements.
    let drift: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT i.name, i.qty_on_hand, COALESCE(SUM(m.qty_delta), 0) FROM item i \
         LEFT JOIN stock_movement m ON m.item_id = i.id GROUP BY i.id \
         HAVING i.qty_on_hand <> COALESCE(SUM(m.qty_delta), 0)",
    )
    .fetch_all(db)
    .await?;
    assert!(drift.is_empty(), "{context}: stock drifted {drift:?}");

    // Document headers equal their lines.
    let mismatched: Vec<i64> = sqlx::query_scalar(
        "SELECT i.id FROM invoice i LEFT JOIN invoice_line l ON l.invoice_id = i.id \
         GROUP BY i.id \
         HAVING i.subtotal_minor <> COALESCE(SUM(l.net_minor), 0) \
             OR i.tax_minor <> COALESCE(SUM(l.tax_minor), 0) \
             OR i.total_minor <> COALESCE(SUM(l.gross_minor), 0) \
             OR i.total_minor <> i.subtotal_minor + i.tax_minor",
    )
    .fetch_all(db)
    .await?;
    assert!(
        mismatched.is_empty(),
        "{context}: totals differ from lines {mismatched:?}"
    );

    // Status follows the money: never overpaid, and paid exactly when nothing is owed.
    let rows = reports::invoice_export_rows(db, None, None).await?;
    for row in &rows {
        assert!(
            row.paid_minor <= row.total_minor.max(0),
            "{context}: overpaid {row:?}"
        );
        let expected = match row.status.as_str() {
            "void" => {
                assert_eq!(row.paid_minor, 0, "{context}: a void invoice kept payments");
                "void"
            }
            _ if row.paid_minor >= row.total_minor => "paid",
            _ if row.paid_minor > 0 => "part_paid",
            _ => "issued",
        };
        assert_eq!(row.status, expected, "{context}: wrong status for {row:?}");
        assert!(
            row.number.is_some(),
            "{context}: issued without a number {row:?}"
        );
    }
    let numbers: std::collections::HashSet<_> = rows.iter().map(|r| r.number.clone()).collect();
    assert_eq!(
        numbers.len(),
        rows.len(),
        "{context}: an invoice number repeats"
    );

    // Every report agrees. With no date range, a void cancels its own sale.
    let live: Vec<_> = rows.iter().filter(|r| r.status != "void").collect();
    let live_total: i64 = live.iter().map(|r| r.total_minor).sum();
    let live_tax: i64 = live.iter().map(|r| r.tax_minor).sum();
    let tax = reports::tax_summary(db, None, None).await?;
    let months = reports::sales_by_month(db, None, None).await?;
    let by_customer = reports::sales_by_customer(db, None, None).await?;
    assert_eq!(
        tax.iter().map(|r| r.gross_minor).sum::<i64>(),
        live_total,
        "{context}: tax summary"
    );
    assert_eq!(
        tax.iter().map(|r| r.tax_minor).sum::<i64>(),
        live_tax,
        "{context}: tax summary tax"
    );
    assert_eq!(
        months.iter().map(|r| r.gross_minor).sum::<i64>(),
        live_total,
        "{context}: monthly sales"
    );
    assert_eq!(
        months.iter().map(|r| r.tax_minor).sum::<i64>(),
        live_tax,
        "{context}: monthly tax"
    );
    assert_eq!(
        months.iter().map(|r| r.invoice_count).sum::<i64>(),
        rows.len() as i64,
        "{context}: monthly invoice count"
    );
    assert_eq!(
        by_customer.iter().map(|r| r.gross_minor).sum::<i64>(),
        live_total,
        "{context}: customer sales"
    );

    // Cash received agrees between the customer report, the payments export and the invoices.
    let received: i64 = reports::payment_export_rows(db, None, None)
        .await?
        .iter()
        .map(|r| r.amount_minor)
        .sum();
    assert_eq!(
        by_customer.iter().map(|r| r.paid_minor).sum::<i64>(),
        received,
        "{context}: cash by customer"
    );
    assert_eq!(
        rows.iter().map(|r| r.paid_minor).sum::<i64>(),
        received,
        "{context}: cash on invoices"
    );
    assert_eq!(
        rows.iter().map(|r| r.balance_minor).sum::<i64>(),
        live.iter()
            .map(|r| (r.total_minor - r.paid_minor).max(0))
            .sum::<i64>(),
        "{context}: balances"
    );
    Ok(())
}

#[sqlx::test]
async fn any_sequence_of_actions_keeps_the_books_consistent(pool: Db) -> Result<(), DataError> {
    let mut business = settings::get(&pool).await?;
    business.business_name = "Invariant Ltd".into();
    settings::update(&pool, &business).await?;
    let mut customer_ids = Vec::new();
    for name in ["Ada", "Grace", "Linus"] {
        customer_ids.push(
            customers::create(
                &pool,
                &customers::CustomerInput {
                    name: name.into(),
                    email: String::new(),
                    phone: String::new(),
                    billing_address: String::new(),
                    notes: String::new(),
                },
            )
            .await?,
        );
    }

    for seed in [1_u64, 7, 42] {
        let mut rng = Lcg(seed);
        for n in 0..120 {
            step(&pool, &mut rng, &customer_ids).await?;
            check(&pool, &format!("seed {seed}, step {n}")).await?;
        }
    }

    // The run exercised every path it is meant to.
    let counts: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM invoice WHERE status = 'void'), \
                (SELECT COUNT(*) FROM invoice WHERE status = 'paid'), \
                (SELECT COUNT(*) FROM payment_removal), \
                (SELECT COUNT(*) FROM stock_movement WHERE reason = 'sale')",
    )
    .fetch_one(&pool)
    .await?;
    assert!(
        counts.0 > 0 && counts.1 > 0 && counts.2 > 0 && counts.3 > 0,
        "the scenario should void, settle, remove payments and sell stock: {counts:?}"
    );
    Ok(())
}
