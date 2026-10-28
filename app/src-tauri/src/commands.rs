//! The Tauri commands: thin wrappers that lock the service and delegate.

use std::path::PathBuf;
use std::sync::Mutex;

use lancelot_core::{AccountId, EventDetails, Parsed};
use tauri::State;

use crate::api::{ApiError, AppInfo, BackupInfo, OpenEvent, ParsedDate, RecentEvent};
use crate::service::Service;
use crate::settings::AppSettings;

pub struct AppState(pub Mutex<Service>);

type Result<T> = std::result::Result<T, ApiError>;

fn with<T>(state: &State<'_, AppState>, f: impl FnOnce(&mut Service) -> Result<T>) -> Result<T> {
    let mut service = state
        .0
        .lock()
        .map_err(|_| ApiError::new("internal", "the app state is unavailable"))?;
    f(&mut service)
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: lancelot_core::VERSION.to_owned(),
        schema_version: lancelot_core::SCHEMA_VERSION,
    }
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<AppSettings> {
    with(&state, |s| Ok(s.settings()))
}

#[tauri::command]
pub async fn settings_set(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings> {
    with(&state, |s| s.update_settings(settings))
}

#[tauri::command]
pub async fn recent_events(state: State<'_, AppState>) -> Result<Vec<RecentEvent>> {
    with(&state, |s| Ok(s.recent_events()))
}

#[tauri::command]
pub async fn recent_forget(state: State<'_, AppState>, path: PathBuf) -> Result<Vec<RecentEvent>> {
    with(&state, |s| s.forget_recent(&path))
}

#[tauri::command]
pub async fn event_create(
    state: State<'_, AppState>,
    path: PathBuf,
    details: EventDetails,
) -> Result<OpenEvent> {
    with(&state, |s| s.create_event(&path, details))
}

#[tauri::command]
pub async fn event_open(state: State<'_, AppState>, path: PathBuf) -> Result<OpenEvent> {
    with(&state, |s| s.open_event(&path))
}

#[tauri::command]
pub async fn file_close(state: State<'_, AppState>) -> Result<()> {
    with(&state, |s| {
        s.close_file();
        Ok(())
    })
}

#[tauri::command]
pub async fn event_current(state: State<'_, AppState>) -> Result<Option<OpenEvent>> {
    with(&state, |s| Ok(s.current()))
}

#[tauri::command]
pub async fn event_update(state: State<'_, AppState>, details: EventDetails) -> Result<OpenEvent> {
    with(&state, |s| s.update_event(details))
}

#[tauri::command]
pub async fn event_set_default_reimbursement_account(
    state: State<'_, AppState>,
    account: AccountId,
) -> Result<OpenEvent> {
    with(&state, |s| s.set_default_reimbursement_account(account))
}

#[tauri::command]
pub async fn integrity_check(state: State<'_, AppState>) -> Result<OpenEvent> {
    with(&state, |s| s.check_integrity())
}

#[tauri::command]
pub async fn backups_list(state: State<'_, AppState>) -> Result<Vec<BackupInfo>> {
    with(&state, |s| s.backups())
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    backup: PathBuf,
    target: PathBuf,
) -> Result<OpenEvent> {
    with(&state, |s| s.restore_backup(&backup, &target))
}

#[tauri::command]
pub async fn amount_parse(
    state: State<'_, AppState>,
    input: String,
    allow_negative: bool,
) -> Result<Parsed> {
    with(&state, |s| s.parse_amount(&input, allow_negative))
}

#[tauri::command]
pub async fn date_parse(state: State<'_, AppState>, input: String) -> Result<ParsedDate> {
    with(&state, |s| s.parse_date(&input))
}
