//! Upgrading a database created by a build from before 0.1.0 (schema 0009, no validation or
//! uniqueness rules) must succeed, keep a pre-upgrade copy, and leave every record usable.

use std::path::PathBuf;

use rmb_data::db::{self, MIGRATOR};
use rmb_data::repos::{jobs, meta, quotes, recurring};
use rmb_data::{backup, DataError};
use sqlx::sqlite::SqlitePoolOptions;

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rmb-upgrade-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Data a pre-0.1.0 build could legitimately have written.
const LEGACY_DATA: &str = r#"
UPDATE settings SET business_name = 'Legacy Plumbing' WHERE id = 1;
INSERT INTO customer (id, name) VALUES (1, 'Acme');

-- Presets applied twice (UK then CA) and a same-named rate with a different percentage.
INSERT INTO tax_rate (id, name, rate_bp, inclusive) VALUES
  (1, 'No Tax', 0, 0), (2, 'VAT 20%', 2000, 0), (3, 'No Tax', 0, 0), (4, 'vat 20%', 1750, 0);

-- Case-variant SKUs; the second item defaults to the duplicate "No Tax".
INSERT INTO item (id, kind, name, sku, default_tax_rate_id) VALUES
  (1, 'product', 'Washer', 'W-1', 1), (2, 'product', 'Washer (bulk)', 'w-1', 3);

-- The same invoice number issued twice after a prefix change.
INSERT INTO invoice (id, customer_id, number, status, issue_date, total_minor) VALUES
  (1, 1, 'INV-0001', 'issued', '2026-01-10', 1000),
  (2, 1, 'INV-0001', 'issued', '2026-02-10', 2000);

-- A double-clicked conversion: two drafts linked to one quote.
INSERT INTO quote (id, customer_id, number, status) VALUES (1, 1, 'Q-0001', 'converted');
INSERT INTO invoice (id, customer_id, status, source_quote_id) VALUES
  (3, 1, 'draft', 1), (4, 1, 'draft', 1);

-- An accepted quote whose lines break today's validation.
INSERT INTO quote (id, customer_id, number, status) VALUES (2, 1, 'Q-0002', 'accepted');
INSERT INTO quote_line (quote_id, description, quantity, unit_price_minor, tax_rate_name,
                        tax_rate_bp, tax_inclusive, net_minor, tax_minor, gross_minor, line_order)
VALUES (2, 'Service', '2', 5000, 'No Tax', 0, 0, 10000, 0, 10000, 0),
       (2, 'Returned part', '-1', 1500, 'No Tax', 0, 0, -1500, 0, -1500, 1),
       (2, '', '0', 999, 'No Tax', 0, 0, 0, 0, 0, 2);

-- A free-text material quantity with a decimal comma.
INSERT INTO job (id, customer_id, title) VALUES (1, 1, 'Leak');
INSERT INTO job_material (job_id, description, quantity, unit_price_minor)
VALUES (1, 'Pipe (m)', '1,5', 1000);

-- A recurring template with a discount line.
INSERT INTO recurring_invoice (id, customer_id, frequency, next_date) VALUES
  (1, 1, 'monthly', '2099-01-01');
INSERT INTO recurring_invoice_line (recurring_id, description, quantity, unit_price_minor,
                                    tax_rate_name, tax_rate_bp, tax_inclusive, line_order)
VALUES (1, 'Retainer', '1', 10000, 'No Tax', 0, 0, 0),
       (1, 'Loyalty discount', '1', -1000, 'No Tax', 0, 0, 1);
"#;

#[tokio::test]
async fn pre_release_database_upgrades_cleanly_and_keeps_a_backup() -> Result<(), DataError> {
    let dir = temp_dir("legacy");
    let db_path = dir.join("rmb.sqlite");
    let backups = dir.join("backups");

    let legacy = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(db::connect_options(&db_path))
        .await?;
    MIGRATOR.run_to(9, &legacy).await?;
    sqlx::raw_sql(LEGACY_DATA).execute(&legacy).await?;
    legacy.close().await;

    let opened = db::open_app(&db_path, &backups).await?;
    let pool = opened.db;
    assert!(!opened.created);
    let saved = opened
        .pre_upgrade_backup
        .expect("an existing database is copied before it is upgraded");
    backup::validate_backup(&saved).await?;

    // Exact duplicate tax rates collapse onto the oldest; a clash with a different rate is renamed.
    let rates: Vec<(i64, String, bool)> =
        sqlx::query_as("SELECT id, name, archived FROM tax_rate ORDER BY id")
            .fetch_all(&pool)
            .await?;
    assert_eq!(
        rates,
        vec![
            (1, "No Tax".into(), false),
            (2, "VAT 20%".into(), false),
            (3, "No Tax".into(), true),
            (4, "vat 20% (4)".into(), false),
        ]
    );
    let defaults: Vec<Option<i64>> =
        sqlx::query_scalar("SELECT default_tax_rate_id FROM item ORDER BY id")
            .fetch_all(&pool)
            .await?;
    assert_eq!(
        defaults,
        vec![Some(1), Some(1)],
        "items move off the archived duplicate"
    );

    let skus: Vec<String> = sqlx::query_scalar("SELECT sku FROM item ORDER BY id")
        .fetch_all(&pool)
        .await?;
    assert_eq!(skus, vec!["W-1", "w-1-2"]);

    let numbers: Vec<Option<String>> =
        sqlx::query_scalar("SELECT number FROM invoice WHERE id IN (1, 2) ORDER BY id")
            .fetch_all(&pool)
            .await?;
    assert_eq!(
        numbers,
        vec![Some("INV-0001".into()), Some("INV-0001-DUP2".into())]
    );
    let links: Vec<Option<i64>> =
        sqlx::query_scalar("SELECT source_quote_id FROM invoice WHERE id IN (3, 4) ORDER BY id")
            .fetch_all(&pool)
            .await?;
    assert_eq!(links, vec![Some(1), None]);

    let notices = meta::take_upgrade_notices(&pool).await?;
    assert_eq!(
        notices.len(),
        3,
        "tax rates, SKUs and invoice numbers: {notices:?}"
    );
    assert!(notices.iter().any(|n| n.contains("INV-0001-DUP2")));
    assert!(
        meta::take_upgrade_notices(&pool).await?.is_empty(),
        "notices show once"
    );

    // Every legacy record is usable again.
    let invoice = quotes::convert_to_invoice(&pool, 2).await?;
    let total: i64 = sqlx::query_scalar("SELECT total_minor FROM invoice WHERE id = ?")
        .bind(invoice)
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        total, 8500,
        "10000 service − 1500 returned part + 0 for the blank line"
    );

    let job = jobs::get_detail(&pool, 1).await?.expect("job loads");
    assert_eq!(job.materials[0].quantity, "1.5");
    assert_eq!(job.materials_total_minor, 1500);

    let schedules = recurring::list(&pool).await?;
    assert_eq!(schedules[0].total_minor, 9000);
    assert!(schedules[0].problem.is_none());

    // A second launch has nothing to migrate, so it takes no extra copy.
    pool.close().await;
    let reopened = db::open_app(&db_path, &backups).await?;
    assert!(reopened.pre_upgrade_backup.is_none());
    reopened.db.close().await;

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[tokio::test]
async fn first_launch_creates_a_database_without_an_upgrade_copy() -> Result<(), DataError> {
    let dir = temp_dir("fresh");
    let opened = db::open_app(&dir.join("rmb.sqlite"), &dir.join("backups")).await?;
    assert!(opened.created);
    assert!(opened.pre_upgrade_backup.is_none());
    assert!(meta::take_upgrade_notices(&opened.db).await?.is_empty());
    opened.db.close().await;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
