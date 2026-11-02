//! The Tauri commands: thin wrappers that lock the service and delegate.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use lancelot_core::{
    AccountDraft, AccountId, AccountKind, AssetType, AttachmentId, BlockerOverride,
    BudgetLineDraft, BudgetLineId, CancellationDraft, CancellationEdit, CancellationId, Cents,
    ChangeLogEntry, CopyChoice, CopySummary, CostCentreId, EventDetails, ImportDeletion, ImportId,
    ImportPreview, ManualSaleDraft, ManualSaleId, Parsed, ParsedSum, Person, PluginChange,
    ReconciliationId, Resolution, Table, TemplateSummary, TicketBatch, TicketShare, TierDraft,
    TierId, TxnDraft, TxnId,
};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use lancelot_service::api::{
    Added, ApiError, AppInfo, BackupInfo, CheckedIban, CountInput, OpenEvent, ParsedDate,
    RecentEvent, SavedTxn,
};
use lancelot_service::{AppSettings, DataSource, DataSourceView, Pulled, Service};

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
    accounts: Vec<AccountDraft>,
    template: Option<PathBuf>,
) -> Result<OpenEvent> {
    with(&state, |s| match &template {
        Some(template) => s.create_event_from(&path, details, template),
        None => s.create_event(&path, details, &accounts),
    })
}

#[tauri::command]
pub async fn template_preview(
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<TemplateSummary> {
    with(&state, |s| s.template_preview(&path))
}

#[tauri::command]
pub async fn copy_preview(state: State<'_, AppState>, path: PathBuf) -> Result<CopySummary> {
    with(&state, |s| s.copy_preview(&path))
}

#[tauri::command]
pub async fn copy_from_event(
    state: State<'_, AppState>,
    path: PathBuf,
    choice: CopyChoice,
) -> Result<OpenEvent> {
    with(&state, |s| s.copy_from_event(&path, choice))
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
    with(&state, |s| s.change(|b| b.update_event(details)))
}

#[tauri::command]
pub async fn event_set_default_reimbursement_account(
    state: State<'_, AppState>,
    account: AccountId,
) -> Result<OpenEvent> {
    with(&state, |s| {
        s.change(|b| b.set_default_reimbursement_account(account))
    })
}

#[tauri::command]
pub async fn integrity_check(state: State<'_, AppState>) -> Result<OpenEvent> {
    with(&state, |s| s.check_integrity())
}

#[tauri::command]
pub async fn change_log_list(state: State<'_, AppState>) -> Result<Vec<ChangeLogEntry>> {
    with(&state, |s| s.change_log())
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
pub async fn amount_sum(
    state: State<'_, AppState>,
    input: String,
    allow_negative: bool,
) -> Result<ParsedSum> {
    with(&state, |s| s.parse_sum(&input, allow_negative))
}

#[tauri::command]
pub async fn ticket_share(
    state: State<'_, AppState>,
    price: Cents,
    bp: u32,
    count: u32,
) -> Result<TicketShare> {
    with(&state, |s| s.ticket_share(price, bp, count))
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
    with(&state, |s| s.change(|b| b.delete_txn(id)))
}

#[tauri::command]
pub async fn account_add(
    state: State<'_, AppState>,
    kind: AccountKind,
    name: String,
) -> Result<Added> {
    with(&state, |s| s.add(|b| b.add_account(kind, &name)))
}

#[tauri::command]
pub async fn asset_add(
    state: State<'_, AppState>,
    name: String,
    asset_type: AssetType,
) -> Result<Added> {
    with(&state, |s| s.add(|b| b.add_asset(&name, asset_type)))
}

#[tauri::command]
pub async fn asset_set_net_movement(
    state: State<'_, AppState>,
    id: AccountId,
    net_movement: bool,
) -> Result<OpenEvent> {
    with(&state, |s| {
        s.change(|b| b.set_net_movement(id, net_movement))
    })
}

#[tauri::command]
pub async fn asset_set_type(
    state: State<'_, AppState>,
    id: AccountId,
    asset_type: AssetType,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.set_asset_type(id, asset_type)))
}

#[tauri::command]
pub async fn counts_set(
    state: State<'_, AppState>,
    account: AccountId,
    opening: Option<CountInput>,
    closing: Option<CountInput>,
) -> Result<OpenEvent> {
    with(&state, |s| s.set_counts(account, opening, closing))
}

#[tauri::command]
pub async fn reconciliation_add(
    state: State<'_, AppState>,
    account: AccountId,
    date: String,
    amount: Cents,
) -> Result<OpenEvent> {
    let date = lancelot_core::parse_date(&date)?;
    with(&state, |s| {
        s.change(|b| b.add_reconciliation(account, date, amount))
    })
}

#[tauri::command]
pub async fn reconciliation_update(
    state: State<'_, AppState>,
    id: ReconciliationId,
    date: String,
    amount: Cents,
) -> Result<OpenEvent> {
    let date = lancelot_core::parse_date(&date)?;
    with(&state, |s| {
        s.change(|b| b.update_reconciliation(id, date, amount))
    })
}

#[tauri::command]
pub async fn reconciliation_delete(
    state: State<'_, AppState>,
    id: ReconciliationId,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_reconciliation(id)))
}

#[tauri::command]
pub async fn account_rename(
    state: State<'_, AppState>,
    id: AccountId,
    name: String,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.rename_account(id, &name)))
}

#[tauri::command]
pub async fn person_add(state: State<'_, AppState>, name: String) -> Result<Added> {
    with(&state, |s| {
        s.add(|b| b.add_person(&name, Person::default()))
    })
}

#[tauri::command]
pub async fn person_update(
    state: State<'_, AppState>,
    id: AccountId,
    name: String,
    person: Person,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.update_person(id, &name, person)))
}

#[tauri::command]
pub async fn budget_line_add(
    state: State<'_, AppState>,
    draft: BudgetLineDraft,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.add_budget_line(draft)))
}

#[tauri::command]
pub async fn budget_line_update(
    state: State<'_, AppState>,
    id: BudgetLineId,
    draft: BudgetLineDraft,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.update_budget_line(id, draft)))
}

#[tauri::command]
pub async fn budget_line_delete(state: State<'_, AppState>, id: BudgetLineId) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_budget_line(id)))
}

#[tauri::command]
pub async fn cost_centre_add(state: State<'_, AppState>, name: String) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.add_cost_centre(&name)))
}

#[tauri::command]
pub async fn cost_centre_rename(
    state: State<'_, AppState>,
    id: CostCentreId,
    name: String,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.rename_cost_centre(id, &name)))
}

#[tauri::command]
pub async fn cost_centre_delete(state: State<'_, AppState>, id: CostCentreId) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_cost_centre(id)))
}

#[tauri::command]
pub async fn category_set_cost_centre(
    state: State<'_, AppState>,
    category: AccountId,
    cost_centre: Option<CostCentreId>,
) -> Result<OpenEvent> {
    with(&state, |s| {
        s.change(|b| b.set_category_cost_centre(category, cost_centre))
    })
}

#[tauri::command]
pub async fn plugin_records_apply(
    state: State<'_, AppState>,
    plugin: String,
    changes: Vec<PluginChange>,
) -> Result<OpenEvent> {
    with(&state, |s| {
        s.change(|b| b.apply_plugin_changes(plugin, changes))
    })
}

#[tauri::command]
pub async fn tier_add(state: State<'_, AppState>, draft: TierDraft) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.add_tier(draft)))
}

#[tauri::command]
pub async fn tier_update(
    state: State<'_, AppState>,
    id: TierId,
    draft: TierDraft,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.update_tier(id, draft)))
}

#[tauri::command]
pub async fn tier_delete(state: State<'_, AppState>, id: TierId) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_tier(id)))
}

#[tauri::command]
pub async fn tickets_set_enabled(state: State<'_, AppState>, enabled: bool) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.set_tickets_enabled(enabled)))
}

#[tauri::command]
pub async fn ticket_account_set(
    state: State<'_, AppState>,
    account: Option<AccountId>,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.set_ticket_account(account)))
}

#[tauri::command]
pub async fn data_sources_get(
    state: State<'_, AppState>,
) -> Result<BTreeMap<String, DataSourceView>> {
    with(&state, |s| Ok(s.data_sources()))
}

#[tauri::command]
pub async fn data_source_set(
    state: State<'_, AppState>,
    id: String,
    source: Option<DataSource>,
) -> Result<BTreeMap<String, DataSourceView>> {
    with(&state, |s| s.set_data_source(&id, source))
}

#[tauri::command]
pub async fn data_source_set_token(
    state: State<'_, AppState>,
    id: String,
    token: Option<String>,
) -> Result<BTreeMap<String, DataSourceView>> {
    with(&state, |s| s.set_data_source_token(&id, token.as_deref()))
}

/// The network is slow, so the service isn't locked meanwhile and the app keeps working.
#[tauri::command]
pub async fn data_source_pull(
    state: State<'_, AppState>,
    id: String,
    fields: Vec<String>,
) -> Result<Pulled> {
    let request = with(&state, |s| s.pull_request(&id))?;
    tauri::async_runtime::spawn_blocking(move || request.run(&fields))
        .await
        .map_err(|e| ApiError::new("internal", e.to_string()))?
}

#[tauri::command]
pub async fn table_read(
    state: State<'_, AppState>,
    path: PathBuf,
    columns: Vec<String>,
) -> Result<Table> {
    with(&state, |s| s.read_table(&path, &columns))
}

#[tauri::command]
pub async fn tickets_preview(
    state: State<'_, AppState>,
    batch: TicketBatch,
    date: String,
) -> Result<ImportPreview> {
    let date = lancelot_core::parse_date(&date)?;
    with(&state, |s| s.preview_tickets(&batch, date))
}

#[tauri::command]
pub async fn tickets_import(
    state: State<'_, AppState>,
    batch: TicketBatch,
    date: String,
) -> Result<OpenEvent> {
    let date = lancelot_core::parse_date(&date)?;
    with(&state, |s| s.import_tickets(&batch, date))
}

#[tauri::command]
pub async fn import_set_date(
    state: State<'_, AppState>,
    id: ImportId,
    date: String,
) -> Result<OpenEvent> {
    let date = lancelot_core::parse_date(&date)?;
    with(&state, |s| s.change(|b| b.set_import_date(id, date)))
}

#[tauri::command]
pub async fn import_delete_preview(
    state: State<'_, AppState>,
    id: ImportId,
) -> Result<ImportDeletion> {
    with(&state, |s| s.preview_delete_import(id))
}

#[tauri::command]
pub async fn import_delete(state: State<'_, AppState>, id: ImportId) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_import(id)))
}

#[tauri::command]
pub async fn account_delete(state: State<'_, AppState>, id: AccountId) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_account(id)))
}

#[tauri::command]
pub async fn manual_sale_add(
    state: State<'_, AppState>,
    draft: ManualSaleDraft,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.add_manual_sale(draft)))
}

#[tauri::command]
pub async fn manual_sale_update(
    state: State<'_, AppState>,
    id: ManualSaleId,
    draft: ManualSaleDraft,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.update_manual_sale(id, draft)))
}

#[tauri::command]
pub async fn manual_sale_delete(state: State<'_, AppState>, id: ManualSaleId) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_manual_sale(id)))
}

#[tauri::command]
pub async fn cancellation_add(
    state: State<'_, AppState>,
    draft: CancellationDraft,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.add_cancellation(draft)))
}

#[tauri::command]
pub async fn cancellation_update(
    state: State<'_, AppState>,
    id: CancellationId,
    edit: CancellationEdit,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.update_cancellation(id, edit)))
}

#[tauri::command]
pub async fn cancellation_delete(
    state: State<'_, AppState>,
    id: CancellationId,
) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.delete_cancellation(id)))
}

#[tauri::command]
pub async fn conflict_resolve(
    state: State<'_, AppState>,
    order: String,
    tier: TierId,
    resolution: Resolution,
) -> Result<OpenEvent> {
    with(&state, |s| {
        s.change(|b| b.resolve_conflict(&order, tier, resolution))
    })
}

#[tauri::command]
pub async fn iban_check(state: State<'_, AppState>, input: String) -> Result<CheckedIban> {
    with(&state, |s| s.check_iban(&input))
}

#[tauri::command]
pub async fn transactions_export_csv(state: State<'_, AppState>, target: PathBuf) -> Result<()> {
    with(&state, |s| s.export_transactions_csv(&target))
}

#[tauri::command]
pub async fn event_close(
    state: State<'_, AppState>,
    overrides: Vec<BlockerOverride>,
) -> Result<OpenEvent> {
    with(&state, |s| s.close_event(overrides))
}

#[tauri::command]
pub async fn event_reopen(state: State<'_, AppState>) -> Result<OpenEvent> {
    with(&state, |s| s.change(|b| b.reopen()))
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
    people: Vec<AccountId>,
) -> Result<OpenEvent> {
    let date = lancelot_core::parse_date(&date)?;
    with(&state, |s| {
        s.change(|b| b.mark_payouts_paid(account, date, &people))
    })
}

/// Opens the print dialog for the window, e.g. to save the payout list as PDF.
#[tauri::command]
pub async fn print_window(window: tauri::WebviewWindow) -> Result<()> {
    window
        .print()
        .map_err(|e| ApiError::new("print_failed", e.to_string()))
}
