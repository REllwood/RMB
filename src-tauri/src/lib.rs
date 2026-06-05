//! RMB Tauri application entry point.
//!
//! Thin shell: registers command handlers and runs the app. Business logic lives in
//! `rmb-domain` (pure) and `rmb-data` (sqlx). Modules grow as the build progresses.

mod commands;
mod error;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
