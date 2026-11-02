//! The commands by name, as `app/src-tauri/src/commands.rs` offers them to the UI.
//! `tests/bridge.rs` checks that none of the app's is missing.

use std::path::PathBuf;

use lancelot_core::{AccountDraft, Book, DomainError, Person, TicketBatch};
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

/// A change to the book that takes one argument.
fn one<A: DeserializeOwned, T>(
    s: &mut Service,
    args: &Value,
    key: &str,
    command: impl FnOnce(&mut Book, A) -> std::result::Result<T, DomainError>,
) -> Result<Value> {
    let first = arg(args, key)?;
    json(s.change(|b| command(b, first)))
}

/// A change to the book that takes two arguments.
fn two<A: DeserializeOwned, B: DeserializeOwned, T>(
    s: &mut Service,
    args: &Value,
    keys: (&str, &str),
    command: impl FnOnce(&mut Book, A, B) -> std::result::Result<T, DomainError>,
) -> Result<Value> {
    let (first, second) = (arg(args, keys.0)?, arg(args, keys.1)?);
    json(s.change(|b| command(b, first, second)))
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
            let accounts: Vec<AccountDraft> = arg(a, "accounts")?;
            json(match arg::<Option<PathBuf>>(a, "template")? {
                Some(template) => s.create_event_from(&path, details, &template),
                None => s.create_event(&path, details, &accounts),
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
        "event_update" => one(s, a, "details", Book::update_event),

        "event_set_default_reimbursement_account" => {
            one(s, a, "account", Book::set_default_reimbursement_account)
        }

        "integrity_check" => json(s.check_integrity()),
        "change_log_list" => json(s.change_log()),
        "backups_list" => json(s.backups(&arg::<PathBuf>(a, "event")?)),
        "backup_restore" => json(s.restore_backup(
            &arg::<PathBuf>(a, "event")?,
            &arg::<PathBuf>(a, "backup")?,
            &arg::<PathBuf>(a, "target")?,
        )),
        "amount_parse" => {
            json(s.parse_amount(&arg::<String>(a, "input")?, arg(a, "allowNegative")?))
        }
        "amount_sum" => json(s.parse_sum(&arg::<String>(a, "input")?, arg(a, "allowNegative")?)),
        "ticket_share" => json(s.ticket_share(arg(a, "price")?, arg(a, "bp")?, arg(a, "count")?)),
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
        "txn_delete" => one(s, a, "id", Book::delete_txn),

        "account_add" => {
            let kind = arg(a, "kind")?;
            let name: String = arg(a, "name")?;
            json(s.add(|b| b.add_account(kind, &name)))
        }

        "asset_add" => {
            let name: String = arg(a, "name")?;
            let asset_type = arg(a, "assetType")?;
            json(s.add(|b| b.add_asset(&name, asset_type)))
        }

        "asset_set_type" => two(s, a, ("id", "assetType"), Book::set_asset_type),
        "asset_set_net_movement" => two(s, a, ("id", "netMovement"), Book::set_net_movement),

        "counts_set" => {
            json(s.set_counts(arg(a, "account")?, arg(a, "opening")?, arg(a, "closing")?))
        }
        "reconciliation_add" => {
            let account = arg(a, "account")?;
            let date = date(a)?;
            let amount = arg(a, "amount")?;
            json(s.change(|b| b.add_reconciliation(account, date, amount)))
        }

        "reconciliation_update" => {
            let id = arg(a, "id")?;
            let date = date(a)?;
            let amount = arg(a, "amount")?;
            json(s.change(|b| b.update_reconciliation(id, date, amount)))
        }

        "reconciliation_delete" => one(s, a, "id", Book::delete_reconciliation),

        "account_rename" => {
            let id = arg(a, "id")?;
            let name: String = arg(a, "name")?;
            json(s.change(|b| b.rename_account(id, &name)))
        }

        "person_add" => {
            let name: String = arg(a, "name")?;
            json(s.add(|b| b.add_person(&name, Person::default())))
        }

        "person_update" => {
            let id = arg(a, "id")?;
            let name: String = arg(a, "name")?;
            let person = arg(a, "person")?;
            json(s.change(|b| b.update_person(id, &name, person)))
        }

        "budget_line_add" => one(s, a, "draft", Book::add_budget_line),

        "budget_line_update" => two(s, a, ("id", "draft"), Book::update_budget_line),

        "budget_line_delete" => one(s, a, "id", Book::delete_budget_line),

        "cost_centre_add" => {
            let name: String = arg(a, "name")?;
            json(s.change(|b| b.add_cost_centre(&name)))
        }

        "cost_centre_rename" => {
            let id = arg(a, "id")?;
            let name: String = arg(a, "name")?;
            json(s.change(|b| b.rename_cost_centre(id, &name)))
        }

        "cost_centre_delete" => one(s, a, "id", Book::delete_cost_centre),

        "category_set_cost_centre" => two(
            s,
            a,
            ("category", "costCentre"),
            Book::set_category_cost_centre,
        ),

        "plugin_records_apply" => two(s, a, ("plugin", "changes"), Book::apply_plugin_changes),

        "tier_add" => one(s, a, "draft", Book::add_tier),

        "tier_update" => two(s, a, ("id", "draft"), Book::update_tier),

        "tier_delete" => one(s, a, "id", Book::delete_tier),

        "tickets_set_enabled" => one(s, a, "enabled", Book::set_tickets_enabled),
        "ticket_account_set" => one(s, a, "account", Book::set_ticket_account),

        "data_sources_get" => json(Ok(s.data_sources())),
        "data_source_set" => json(s.set_data_source(&arg::<String>(a, "id")?, arg(a, "source")?)),
        "data_source_set_token" => json(s.set_data_source_token(
            &arg::<String>(a, "id")?,
            arg::<Option<String>>(a, "token")?.as_deref(),
        )),
        "data_source_pull" => json(s.pull_request(&arg::<String>(a, "id")?)?.run(&arg::<
            Vec<String>,
        >(
            a, "fields"
        )?)),
        "table_read" => json(s.read_table(
            &arg::<PathBuf>(a, "path")?,
            &arg::<Vec<String>>(a, "columns")?,
        )),
        "tickets_preview" => json(s.preview_tickets(&arg::<TicketBatch>(a, "batch")?, date(a)?)),
        "tickets_import" => json(s.import_tickets(&arg::<TicketBatch>(a, "batch")?, date(a)?)),
        "import_set_date" => {
            let id = arg(a, "id")?;
            let date = date(a)?;
            json(s.change(|b| b.set_import_date(id, date)))
        }

        "import_delete_preview" => json(s.preview_delete_import(arg(a, "id")?)),
        "import_delete" => one(s, a, "id", Book::delete_import),

        "account_delete" => one(s, a, "id", Book::delete_account),

        "manual_sale_add" => one(s, a, "draft", Book::add_manual_sale),

        "manual_sale_update" => two(s, a, ("id", "draft"), Book::update_manual_sale),

        "manual_sale_delete" => one(s, a, "id", Book::delete_manual_sale),

        "cancellation_add" => one(s, a, "draft", Book::add_cancellation),

        "cancellation_update" => two(s, a, ("id", "edit"), Book::update_cancellation),

        "cancellation_delete" => one(s, a, "id", Book::delete_cancellation),

        "conflict_resolve" => {
            let order: String = arg(a, "order")?;
            let tier = arg(a, "tier")?;
            let resolution = arg(a, "resolution")?;
            json(s.change(|b| b.resolve_conflict(&order, tier, resolution)))
        }

        "iban_check" => json(s.check_iban(&arg::<String>(a, "input")?)),
        "transactions_export_csv" => json(s.export_transactions_csv(&arg::<PathBuf>(a, "target")?)),
        "event_close" => json(s.close_event(arg(a, "overrides")?)),
        "event_reopen" => json(s.change(|b| b.reopen())),

        "payouts_export_csv" => json(s.export_payouts_csv(&arg::<PathBuf>(a, "target")?)),
        "payouts_mark_paid" => {
            let account = arg(a, "account")?;
            let date = date(a)?;
            json(s.change(|b| b.mark_payouts_paid(account, date)))
        }

        _ => Err(ApiError::new(
            "unknown_command",
            format!("the dev bridge has no command {command}"),
        )),
    }
}
