//! The Tauri commands: thin wrappers that lock the service and delegate.

use std::path::PathBuf;
use std::sync::Mutex;

use lancelot_core::{
    AccountId, AccountKind, AttachmentId, EventDetails, Parsed, Person, TxnDraft, TxnId,
};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::api::{
    Added, ApiError, AppInfo, BackupInfo, CheckedIban, OpenEvent, ParsedDate, RecentEvent, SavedTxn,
};
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
pub async fn backups_list(state: State<'_, AppState>, event: PathBuf) -> Result<Vec<BackupInfo>> {
    with(&state, |s| s.backups(&event))
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    event: PathBuf,
    backup: PathBuf,
    target: PathBuf,
) -> Result<OpenEvent> {
    with(&state, |s| s.restore_backup(&event, &backup, &target))
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

#[tauri::command]
pub async fn txn_save(
    state: State<'_, AppState>,
    draft: TxnDraft,
    id: Option<TxnId>,
    files: Option<Vec<PathBuf>>,
    remove: Option<Vec<AttachmentId>>,
) -> Result<SavedTxn> {
    with(&state, |s| {
        s.save_txn(
            &draft,
            id,
            &files.unwrap_or_default(),
            &remove.unwrap_or_default(),
        )
    })
}

/// Opens an attachment in its default app (R-ATT-3).
#[tauri::command]
pub async fn attachment_open(
    app: AppHandle,
    state: State<'_, AppState>,
    id: AttachmentId,
) -> Result<()> {
    let path = with(&state, |s| s.attachment_temp(id))?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| ApiError::new("open_failed", e.to_string()))
}

#[tauri::command]
pub async fn attachment_export(
    state: State<'_, AppState>,
    id: AttachmentId,
    target: PathBuf,
) -> Result<()> {
    with(&state, |s| s.export_attachment(id, &target))
}

#[tauri::command]
pub async fn txn_delete(state: State<'_, AppState>, id: TxnId) -> Result<OpenEvent> {
    with(&state, |s| s.delete_txn(id))
}

#[tauri::command]
pub async fn account_add(
    state: State<'_, AppState>,
    kind: AccountKind,
    name: String,
) -> Result<Added> {
    with(&state, |s| s.add_account(kind, &name))
}

#[tauri::command]
pub async fn account_rename(
    state: State<'_, AppState>,
    id: AccountId,
    name: String,
) -> Result<OpenEvent> {
    with(&state, |s| s.rename_account(id, &name))
}

#[tauri::command]
pub async fn person_add(state: State<'_, AppState>, name: String) -> Result<Added> {
    with(&state, |s| s.add_person(&name))
}

#[tauri::command]
pub async fn person_update(
    state: State<'_, AppState>,
    id: AccountId,
    name: String,
    person: Person,
) -> Result<OpenEvent> {
    with(&state, |s| s.update_person(id, &name, person))
}

#[tauri::command]
pub async fn iban_check(state: State<'_, AppState>, input: String) -> Result<CheckedIban> {
    with(&state, |s| s.check_iban(&input))
}

#[tauri::command]
pub async fn payouts_export_csv(state: State<'_, AppState>, target: PathBuf) -> Result<()> {
    with(&state, |s| s.export_payouts_csv(&target))
}

#[tauri::command]
pub async fn payouts_mark_paid(
    state: State<'_, AppState>,
    account: AccountId,
    date: String,
) -> Result<OpenEvent> {
    let date = lancelot_core::parse_date(&date)?;
    with(&state, |s| s.mark_payouts_paid(account, date))
}

/// Opens the print dialog for the window, e.g. to save the payout list as PDF.
#[tauri::command]
pub async fn print_window(window: tauri::WebviewWindow) -> Result<()> {
    window
        .print()
        .map_err(|e| ApiError::new("print_failed", e.to_string()))
}
