//! RMB Tauri application entry point.
//!
//! Thin shell: opens the local database, registers command handlers, and runs the app.
//! Business logic lives in `rmb-domain` (pure) and `rmb-data` (sqlx). Modules grow as the
//! build progresses.

use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

mod commands;
mod error;
mod pdf;
mod startup;

/// Show why the app can't start, then quit when the user dismisses it. The window is hidden so the
/// half-initialised UI is never used without a database.
fn report_fatal_startup(app: &tauri::App, message: String) {
    eprintln!("startup failed: {message}");
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let handle = app.handle().clone();
    app.dialog()
        .message(message)
        .title("RMB couldn't start")
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::Ok)
        .show(move |_| handle.exit(1));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Two RMB processes on one database can lose work during a restore; a second launch just
        // brings the open window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = match app.path().app_data_dir() {
                Ok(dir) => dir,
                Err(error) => {
                    report_fatal_startup(
                        app,
                        format!(
                            "RMB couldn't find your user profile's application data folder.\n\n{error}"
                        ),
                    );
                    return Ok(());
                }
            };
            match tauri::async_runtime::block_on(startup::initialise(&data_dir)) {
                Ok(pool) => {
                    app.manage(pool.clone());
                    // The launch backup and recurring generation don't hold up the window; the
                    // UI refreshes when they finish.
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        startup::finish_in_background(&pool, &data_dir).await;
                        let _ = handle.emit("startup-complete", ());
                    });
                }
                Err(message) => report_fatal_startup(app, message),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::get_meta,
            commands::set_meta,
            commands::backup::backup_database,
            commands::backup::restore_database,
            commands::backup::backup_folder,
            commands::dismiss_startup_warning,
            // settings + tax
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::list_tax_rates,
            commands::settings::create_tax_rate,
            commands::settings::update_tax_rate,
            commands::settings::archive_tax_rate,
            commands::settings::list_archived_tax_rates,
            commands::settings::restore_tax_rate,
            commands::settings::apply_tax_preset,
            commands::settings::set_logo,
            commands::settings::clear_logo,
            // customers
            commands::customers::list_customers,
            commands::customers::get_customer,
            commands::customers::create_customer,
            commands::customers::update_customer,
            commands::customers::delete_customer,
            commands::customers::customer_history,
            // catalog + stock
            commands::catalog::list_items,
            commands::catalog::get_item,
            commands::catalog::create_item,
            commands::catalog::update_item,
            commands::catalog::delete_item,
            commands::catalog::list_archived_items,
            commands::catalog::restore_item,
            commands::catalog::adjust_stock,
            commands::catalog::item_movements,
            // invoices + payments
            commands::invoicing::list_invoices,
            commands::invoicing::get_invoice,
            commands::invoicing::create_invoice,
            commands::invoicing::update_invoice_draft,
            commands::invoicing::delete_invoice_draft,
            commands::invoicing::issue_invoice,
            commands::invoicing::void_invoice,
            commands::invoicing::record_payment,
            commands::invoicing::invoice_payments,
            commands::invoicing::delete_payment,
            commands::invoicing::dashboard_summary,
            // quotes
            commands::quoting::list_quotes,
            commands::quoting::get_quote,
            commands::quoting::create_quote,
            commands::quoting::update_quote_draft,
            commands::quoting::set_quote_status,
            commands::quoting::delete_quote,
            commands::quoting::convert_quote_to_invoice,
            // jobs + timekeeping
            commands::jobs::list_jobs,
            commands::jobs::get_job,
            commands::jobs::create_job,
            commands::jobs::convert_quote_to_job,
            commands::jobs::set_job_status,
            commands::jobs::delete_job,
            commands::jobs::add_time_entry,
            commands::jobs::add_job_material,
            commands::jobs::delete_time_entry,
            commands::jobs::delete_job_material,
            commands::jobs::invoice_job,
            // recurring invoices
            commands::recurring::list_recurring,
            commands::recurring::get_recurring,
            commands::recurring::create_recurring,
            commands::recurring::update_recurring,
            commands::recurring::set_recurring_active,
            commands::recurring::recurring_resume_preview,
            commands::recurring::delete_recurring,
            commands::recurring::run_recurring_now,
            // reports + exports
            commands::reports::report_tax_summary,
            commands::reports::report_sales_monthly,
            commands::reports::report_sales_customers,
            commands::reports::export_invoices_csv,
            commands::reports::export_payments_csv,
            commands::reports::export_customers_csv,
            // pdf
            commands::pdf::export_invoice_pdf,
            commands::pdf::export_quote_pdf,
            commands::pdf::export_receipt_pdf
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
