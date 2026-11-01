//! Tauri shell: command handlers, dialogs and the window ↔ open-event mapping.
//! No money logic lives here; everything goes through `lancelot_core`.

mod commands;
mod tokens;

use std::sync::Mutex;

use tauri::menu::{Menu, MenuItemBuilder, SubmenuBuilder};
use tauri::{App, Emitter, Manager};

use commands::AppState;
use lancelot_service::Service;

fn attachment_response(app: &tauri::AppHandle, path: &str) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Response, StatusCode, header};
    let not_found = || {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap_or_default()
    };
    // `convertFileSrc` encodes the slash, so accept both `/attachment/12` and `/attachment%2F12`.
    let decoded = path.trim_start_matches('/').replace("%2F", "/");
    let Some(id) = decoded
        .strip_prefix("attachment/")
        .and_then(|n| n.parse::<i64>().ok())
    else {
        return not_found();
    };
    let state = app.state::<AppState>();
    let found = state
        .0
        .lock()
        .ok()
        .and_then(|s| s.attachment(lancelot_core::AttachmentId(id)).ok());
    match found {
        Some((mime, bytes)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "no-store")
            .body(bytes)
            .unwrap_or_else(|_| not_found()),
        None => not_found(),
    }
}

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
            app.manage(AppState(Mutex::new(Service::with_tokens(
                data_dir,
                Box::new(tokens::Keychain),
            ))));
            app.set_menu(menu(app)?)?;
            // The UI decides what each menu item does (see app/src/lib/shortcuts.ts).
            app.on_menu_event(|app, event| {
                let _ = app.emit("menu", event.id().0.as_str());
            });
            Ok(())
        })
        // Attachments for previews: lancelot://localhost/attachment/<id> (R-ATT-3).
        .register_uri_scheme_protocol("lancelot", |ctx, request| {
            attachment_response(ctx.app_handle(), request.uri().path())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::settings_get,
            commands::settings_set,
            commands::recent_events,
            commands::recent_forget,
            commands::event_create,
            commands::template_preview,
            commands::copy_preview,
            commands::copy_from_event,
            commands::event_open,
            commands::file_close,
            commands::event_current,
            commands::event_update,
            commands::event_set_default_reimbursement_account,
            commands::integrity_check,
            commands::backups_list,
            commands::backup_restore,
            commands::amount_parse,
            commands::ticket_share,
            commands::date_parse,
            commands::txn_save,
            commands::txn_delete,
            commands::account_add,
            commands::account_rename,
            commands::asset_add,
            commands::asset_set_type,
            commands::asset_set_net_movement,
            commands::counts_set,
            commands::reconciliation_add,
            commands::reconciliation_update,
            commands::reconciliation_delete,
            commands::person_add,
            commands::person_update,
            commands::iban_check,
            commands::budget_line_add,
            commands::budget_line_update,
            commands::budget_line_delete,
            commands::cost_centre_add,
            commands::cost_centre_rename,
            commands::cost_centre_delete,
            commands::category_set_cost_centre,
            commands::plugin_records_apply,
            commands::tier_add,
            commands::tier_update,
            commands::tier_delete,
            commands::data_sources_get,
            commands::data_source_set,
            commands::data_source_set_token,
            commands::data_source_pull,
            commands::table_read,
            commands::tickets_set_enabled,
            commands::ticket_account_set,
            commands::tickets_preview,
            commands::tickets_import,
            commands::import_set_date,
            commands::import_delete_preview,
            commands::import_delete,
            commands::account_delete,
            commands::manual_sale_add,
            commands::manual_sale_update,
            commands::manual_sale_delete,
            commands::cancellation_add,
            commands::cancellation_update,
            commands::cancellation_delete,
            commands::conflict_resolve,
            commands::payouts_export_csv,
            commands::transactions_export_csv,
            commands::event_close,
            commands::event_reopen,
            commands::payouts_mark_paid,
            commands::print_window,
            commands::attachment_open,
            commands::attachment_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lancelot");
}
