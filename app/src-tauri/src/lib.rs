//! Tauri shell: command handlers, dialogs and the window ↔ open-event mapping.
//! No money logic lives here; everything goes through `lancelot_core`.

mod api;
mod commands;
mod service;
mod settings;

use std::sync::Mutex;

use tauri::Manager;

use commands::AppState;
use service::Service;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState(Mutex::new(Service::new(data_dir))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::settings_get,
            commands::settings_set,
            commands::recent_events,
            commands::recent_forget,
            commands::event_create,
            commands::event_open,
            commands::event_close,
            commands::event_current,
            commands::event_update,
            commands::event_set_default_reimbursement_account,
            commands::integrity_check,
            commands::backups_list,
            commands::backup_restore,
            commands::amount_parse,
            commands::date_parse,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lancelot");
}
