//! Tauri shell: command handlers, dialogs and the window ↔ open-event mapping.
//! No money logic lives here; everything goes through `lancelot_core`.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    schema_version: u32,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        version: lancelot_core::VERSION,
        schema_version: lancelot_core::SCHEMA_VERSION,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Lancelot");
}
