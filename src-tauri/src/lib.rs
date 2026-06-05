//! RMB Tauri application entry point.
//!
//! Thin shell: opens the local database, registers command handlers, and runs the app.
//! Business logic lives in `rmb-domain` (pure) and `rmb-data` (sqlx). Modules grow as the
//! build progresses.

use tauri::Manager;

mod commands;
mod error;

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
            commands::invoicing::dashboard_summary
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
