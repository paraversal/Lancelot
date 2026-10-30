//! The commands by name, as `app/src-tauri/src/commands.rs` offers them to the UI. One line per
//! command; `tests/bridge.rs` checks that none of the app's is missing.

use std::path::PathBuf;

use lancelot_service::Service;
use lancelot_service::api::{ApiError, AppInfo};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

type Result<T> = std::result::Result<T, ApiError>;

/// An argument by the name the UI sends. A missing one reads as `null`, as with Tauri.
fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| ApiError::new("bad_request", format!("argument {key}: {e}")))
}

fn json<T: Serialize>(result: Result<T>) -> Result<Value> {
    serde_json::to_value(result?).map_err(|e| ApiError::new("internal", e.to_string()))
}

fn date(args: &Value) -> Result<time::Date> {
    let text: String = arg(args, "date")?;
    Ok(lancelot_core::parse_date(&text)?)
}

pub fn run(s: &mut Service, command: &str, a: &Value) -> Result<Value> {
    match command {
        "app_info" => json(Ok(AppInfo {
            version: lancelot_core::VERSION.to_owned(),
            schema_version: lancelot_core::SCHEMA_VERSION,
        })),
        "settings_get" => json(Ok(s.settings())),
        "settings_set" => json(s.update_settings(arg(a, "settings")?)),
        "recent_events" => json(Ok(s.recent_events())),
        "recent_forget" => json(s.forget_recent(&arg::<PathBuf>(a, "path")?)),
        "event_create" => {
            let path: PathBuf = arg(a, "path")?;
            let details = arg(a, "details")?;
            json(match arg::<Option<PathBuf>>(a, "template")? {
                Some(template) => s.create_event_from(&path, details, &template),
                None => s.create_event(&path, details),
            })
        }
        "template_preview" => json(s.template_preview(&arg::<PathBuf>(a, "path")?)),
        "copy_preview" => json(s.copy_preview(&arg::<PathBuf>(a, "path")?)),
        "copy_from_event" => {
            json(s.copy_from_event(&arg::<PathBuf>(a, "path")?, arg(a, "choice")?))
        }
        "event_open" => json(s.open_event(&arg::<PathBuf>(a, "path")?)),
        "file_close" => {
            s.close_file();
            Ok(Value::Null)
        }
        "event_current" => json(Ok(s.current())),
        "event_update" => json(s.update_event(arg(a, "details")?)),
        "event_set_default_reimbursement_account" => {
            json(s.set_default_reimbursement_account(arg(a, "account")?))
        }
        "integrity_check" => json(s.check_integrity()),
        "backups_list" => json(s.backups(&arg::<PathBuf>(a, "event")?)),
        "backup_restore" => json(s.restore_backup(
            &arg::<PathBuf>(a, "event")?,
            &arg::<PathBuf>(a, "backup")?,
            &arg::<PathBuf>(a, "target")?,
        )),
        "amount_parse" => {
            json(s.parse_amount(&arg::<String>(a, "input")?, arg(a, "allowNegative")?))
        }
        "date_parse" => json(s.parse_date(&arg::<String>(a, "input")?)),
        "txn_save" => json(s.save_txn(
            &arg(a, "draft")?,
            arg(a, "id")?,
            &arg::<Option<Vec<PathBuf>>>(a, "files")?.unwrap_or_default(),
            &arg::<Option<Vec<_>>>(a, "remove")?.unwrap_or_default(),
        )),
        // The browser has no app to open it in; the file is still written, as in the app.
        "attachment_open" => s.attachment_temp(arg(a, "id")?).map(|_| Value::Null),
        "attachment_export" => {
            json(s.export_attachment(arg(a, "id")?, &arg::<PathBuf>(a, "target")?))
        }
        "txn_delete" => json(s.delete_txn(arg(a, "id")?)),
        "account_add" => json(s.add_account(arg(a, "kind")?, &arg::<String>(a, "name")?)),
        "asset_add" => json(s.add_asset(&arg::<String>(a, "name")?, arg(a, "assetType")?)),
        "asset_set_type" => json(s.set_asset_type(arg(a, "id")?, arg(a, "assetType")?)),
        "counts_set" => {
            json(s.set_counts(arg(a, "account")?, arg(a, "opening")?, arg(a, "closing")?))
        }
        "reconciliation_add" => json(s.add_reconciliation(
            arg(a, "account")?,
            &arg::<String>(a, "date")?,
            arg(a, "amount")?,
        )),
        "reconciliation_update" => json(s.update_reconciliation(
            arg(a, "id")?,
            &arg::<String>(a, "date")?,
            arg(a, "amount")?,
        )),
        "reconciliation_delete" => json(s.delete_reconciliation(arg(a, "id")?)),
        "account_rename" => json(s.rename_account(arg(a, "id")?, &arg::<String>(a, "name")?)),
        "person_add" => json(s.add_person(&arg::<String>(a, "name")?)),
        "person_update" => {
            json(s.update_person(arg(a, "id")?, &arg::<String>(a, "name")?, arg(a, "person")?))
        }
        "budget_line_add" => json(s.add_budget_line(arg(a, "draft")?)),
        "budget_line_update" => json(s.update_budget_line(arg(a, "id")?, arg(a, "draft")?)),
        "budget_line_delete" => json(s.delete_budget_line(arg(a, "id")?)),
        "cost_centre_add" => json(s.add_cost_centre(&arg::<String>(a, "name")?)),
        "cost_centre_rename" => {
            json(s.rename_cost_centre(arg(a, "id")?, &arg::<String>(a, "name")?))
        }
        "cost_centre_delete" => json(s.delete_cost_centre(arg(a, "id")?)),
        "category_set_cost_centre" => {
            json(s.set_category_cost_centre(arg(a, "category")?, arg(a, "costCentre")?))
        }
        "drink_product_add" => json(s.add_drink_product(arg(a, "draft")?)),
        "drink_product_update" => json(s.update_drink_product(arg(a, "id")?, arg(a, "draft")?)),
        "drink_product_delete" => json(s.delete_drink_product(arg(a, "id")?)),
        "drink_settings_set" => json(s.set_drink_settings(arg(a, "settings")?)),
        "tier_add" => json(s.add_tier(arg(a, "draft")?)),
        "tier_update" => json(s.update_tier(arg(a, "id")?, arg(a, "draft")?)),
        "tier_delete" => json(s.delete_tier(arg(a, "id")?)),
        "ticket_columns_set" => json(s.set_ticket_columns(arg(a, "columns")?)),
        "tickets_preview" => json(s.preview_tickets(&arg::<PathBuf>(a, "path")?, date(a)?)),
        "tickets_import" => json(s.import_tickets(
            &arg::<PathBuf>(a, "path")?,
            date(a)?,
            &arg::<String>(a, "sha256")?,
        )),
        "import_set_date" => json(s.set_import_date(arg(a, "id")?, date(a)?)),
        "import_delete_preview" => json(s.preview_delete_import(arg(a, "id")?)),
        "import_delete" => json(s.delete_import(arg(a, "id")?)),
        "account_delete" => json(s.delete_account(arg(a, "id")?)),
        "manual_sale_add" => json(s.add_manual_sale(arg(a, "draft")?)),
        "manual_sale_update" => json(s.update_manual_sale(arg(a, "id")?, arg(a, "draft")?)),
        "manual_sale_delete" => json(s.delete_manual_sale(arg(a, "id")?)),
        "cancellation_add" => json(s.add_cancellation(arg(a, "draft")?)),
        "cancellation_update" => json(s.update_cancellation(arg(a, "id")?, arg(a, "edit")?)),
        "cancellation_delete" => json(s.delete_cancellation(arg(a, "id")?)),
        "conflict_resolve" => json(s.resolve_conflict(
            &arg::<String>(a, "order")?,
            arg(a, "tier")?,
            arg(a, "resolution")?,
        )),
        "iban_check" => json(s.check_iban(&arg::<String>(a, "input")?)),
        "transactions_export_csv" => json(s.export_transactions_csv(&arg::<PathBuf>(a, "target")?)),
        "event_close" => json(s.close_event(arg(a, "overrides")?)),
        "event_reopen" => json(s.reopen_event()),
        "payouts_export_csv" => json(s.export_payouts_csv(&arg::<PathBuf>(a, "target")?)),
        "payouts_mark_paid" => json(s.mark_payouts_paid(arg(a, "account")?, date(a)?)),
        _ => Err(ApiError::new(
            "unknown_command",
            format!("the dev bridge has no command {command}"),
        )),
    }
}
