//! Tauri shell: command handlers, dialogs and the window ↔ open-event mapping.
//! No money logic lives here; everything goes through `lancelot_core`.

mod api;
mod commands;
mod service;
mod settings;

use std::sync::Mutex;

use tauri::menu::{Menu, MenuItemBuilder, SubmenuBuilder};
use tauri::{App, Emitter, Manager};

use commands::AppState;
use service::Service;

/// The native menu. Its own items send a `menu` event with their id to the UI; the standard
/// items (copy, paste, quit, …) work as everywhere else.
fn menu(app: &App) -> tauri::Result<Menu<tauri::Wry>> {
    let item = |id: &str, label: &str, keys: &str| {
        MenuItemBuilder::with_id(id, label)
            .accelerator(keys)
            .build(app)
    };
    let lancelot = SubmenuBuilder::new(app, "Lancelot")
        .about(None)
        .separator()
        .item(&item("settings", "Settings…", "CmdOrCtrl+,")?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&item("new_event", "New Event…", "CmdOrCtrl+Shift+N")?)
        .item(&item("open_event", "Open…", "CmdOrCtrl+O")?)
        .separator()
        .item(&item("close_file", "Close", "CmdOrCtrl+W")?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .build()?;
    Menu::with_items(app, &[&lancelot, &file, &edit, &window])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(AppState(Mutex::new(Service::new(data_dir))));
            app.set_menu(menu(app)?)?;
            // The UI decides what each menu item does (see app/src/lib/shortcuts.ts).
            app.on_menu_event(|app, event| {
                let _ = app.emit("menu", event.id().0.as_str());
            });
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
            commands::file_close,
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
