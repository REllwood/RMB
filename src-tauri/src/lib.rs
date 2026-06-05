//! RMB Tauri application entry point.
//!
//! Thin shell: opens the local database, registers command handlers, and runs the app.
//! Business logic lives in `rmb-domain` (pure) and `rmb-data` (sqlx). Modules grow as the
//! build progresses.

use tauri::Manager;

mod commands;
mod error;
mod pdf;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Open (creating on first run) the local SQLite database, run migrations, and
            // share the pool with every command via managed state.
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("rmb.sqlite");
            let pool = tauri::async_runtime::block_on(rmb_data::db::open(&db_path))?;
            app.manage(pool);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_meta,
            commands::set_meta,
            commands::backup::backup_database,
            commands::backup::restore_database,
            // settings + tax
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::list_tax_rates,
            commands::settings::create_tax_rate,
            commands::settings::update_tax_rate,
            commands::settings::archive_tax_rate,
            commands::settings::apply_tax_preset,
            // customers
            commands::customers::list_customers,
            commands::customers::get_customer,
            commands::customers::create_customer,
            commands::customers::update_customer,
            commands::customers::delete_customer,
            // catalog + stock
            commands::catalog::list_items,
            commands::catalog::get_item,
            commands::catalog::create_item,
            commands::catalog::update_item,
            commands::catalog::delete_item,
            commands::catalog::adjust_stock,
            commands::catalog::item_movements,
            // invoices + payments
            commands::invoicing::list_invoices,
            commands::invoicing::get_invoice,
            commands::invoicing::create_invoice,
            commands::invoicing::issue_invoice,
            commands::invoicing::void_invoice,
            commands::invoicing::record_payment,
            commands::invoicing::invoice_payments,
            commands::invoicing::dashboard_summary,
            // quotes
            commands::quoting::list_quotes,
            commands::quoting::get_quote,
            commands::quoting::create_quote,
            commands::quoting::set_quote_status,
            commands::quoting::delete_quote,
            commands::quoting::convert_quote_to_invoice,
            // jobs + timekeeping
            commands::jobs::list_jobs,
            commands::jobs::get_job,
            commands::jobs::create_job,
            commands::jobs::set_job_status,
            commands::jobs::delete_job,
            commands::jobs::add_time_entry,
            commands::jobs::add_job_material,
            commands::jobs::delete_time_entry,
            commands::jobs::delete_job_material,
            commands::jobs::invoice_job,
            // pdf
            commands::pdf::export_invoice_pdf,
            commands::pdf::export_quote_pdf
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
