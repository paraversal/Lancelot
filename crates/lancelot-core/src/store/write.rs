//! Writes the difference between two books, with one change log entry per changed row.
//!
//! Every command goes through [`persist`]: the store applies the command to a copy of the
//! book, and this module writes whatever differs. That keeps the change log complete without
//! every command having to know which rows it touches.

use std::collections::BTreeMap;

use rusqlite::{Connection, params};
use serde::Serialize;
use time::OffsetDateTime;

use super::StoreError;
use super::codec::{date_text, enum_text, timestamp_text};
use crate::book::Book;
use crate::model::{AttachmentId, Estimate, Origin, Txn};

enum Change<'a, V> {
    Create(&'a V),
    Update(&'a V, &'a V),
    Delete(&'a V),
}

fn diff<'a, V: PartialEq>(
    before: impl IntoIterator<Item = (i64, &'a V)>,
    after: impl IntoIterator<Item = (i64, &'a V)>,
) -> Vec<(i64, Change<'a, V>)> {
    let before: BTreeMap<i64, &V> = before.into_iter().collect();
    let after: BTreeMap<i64, &V> = after.into_iter().collect();
    let mut out = Vec::new();
    for (k, a) in &after {
        match before.get(k) {
            None => out.push((*k, Change::Create(*a))),
            Some(b) if b != a => out.push((*k, Change::Update(*b, *a))),
            Some(_) => {}
        }
    }
    for (k, b) in &before {
        if !after.contains_key(k) {
            out.push((*k, Change::Delete(*b)));
        }
    }
    out
}

struct Log<'c> {
    conn: &'c Connection,
    at: String,
}

impl Log<'_> {
    fn write<V: Serialize>(
        &self,
        entity: &str,
        id: i64,
        change: &Change<'_, V>,
    ) -> Result<(), StoreError> {
        let json = |v: &V| serde_json::to_string(v);
        let (op, before, after) = match change {
            Change::Create(a) => ("create", None, Some(json(a)?)),
            Change::Update(b, a) => ("update", Some(json(b)?), Some(json(a)?)),
            Change::Delete(b) => ("delete", Some(json(b)?), None),
        };
        self.raw(entity, id, op, before, after)
    }

    fn raw(
        &self,
        entity: &str,
        id: i64,
        op: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO change_log (at, entity, entity_id, op, before, after) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![self.at, entity, id, op, before, after],
        )?;
        Ok(())
    }
}

/// Compares one table's rows in two books, runs `write` for each changed row and logs the change.
/// `None` is an empty file. Rows are `(id, value)`.
fn table<'a, V, I>(
    log: &Log<'_>,
    entity: &str,
    before: Option<&'a Book>,
    after: &'a Book,
    rows: impl Fn(&'a Book) -> I,
    mut write: impl FnMut(i64, &Change<'a, V>) -> Result<(), StoreError>,
) -> Result<(), StoreError>
where
    V: PartialEq + Serialize + 'a,
    I: Iterator<Item = (i64, &'a V)>,
{
    for (id, change) in diff(before.map(&rows).into_iter().flatten(), rows(after)) {
        write(id, &change)?;
        log.write(entity, id, &change)?;
    }
    Ok(())
}

/// [`table`] for the usual row: one statement that writes it, one that deletes it.
fn saved<'a, V, I>(
    log: &Log<'_>,
    entity: &str,
    delete: &str,
    before: Option<&'a Book>,
    after: &'a Book,
    rows: impl Fn(&'a Book) -> I,
    mut write: impl FnMut(i64, &'a V) -> Result<(), StoreError>,
) -> Result<(), StoreError>
where
    V: PartialEq + Serialize + 'a,
    I: Iterator<Item = (i64, &'a V)>,
{
    table(
        log,
        entity,
        before,
        after,
        rows,
        |id, change| match change {
            Change::Create(v) | Change::Update(_, v) => write(id, v),
            Change::Delete(_) => {
                log.conn.execute(delete, [id])?;
                Ok(())
            }
        },
    )
}

/// Writes everything in `after` that differs from `before` (`None`: an empty file).
/// `blobs` holds the bytes of the attachments the command added, by their new ids.
///
/// The order of the tables does not matter: foreign keys are enforced at the commit.
pub(crate) fn persist<'a>(
    conn: &Connection,
    before: Option<&'a Book>,
    after: &'a Book,
    blobs: &BTreeMap<AttachmentId, &[u8]>,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    conn.pragma_update(None, "defer_foreign_keys", true)?;
    let log = &Log {
        conn,
        at: timestamp_text(now)?,
    };

    saved(
        log,
        "cost_centre",
        "DELETE FROM cost_centre WHERE id = ?1",
        before,
        after,
        |b| b.cost_centres().map(|c| (c.id.0, c)),
        |id, c| {
            conn.execute(
                "INSERT INTO cost_centre (id, name) VALUES (?1, ?2)
                         ON CONFLICT (id) DO UPDATE SET name = excluded.name",
                params![id, c.name],
            )?;
            Ok(())
        },
    )?;

    table(
        log,
        "account",
        before,
        after,
        |b| b.accounts().map(|a| (a.id.0, a)),
        |id, change| {
            match change {
                Change::Create(a) => {
                    let role = a.role.as_ref().map(enum_text).transpose()?;
                    let asset_type = a.asset_type.as_ref().map(enum_text).transpose()?;
                    conn.execute(
                        "INSERT INTO account (id, kind, name, role, asset_type, net_movement, cost_centre_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            id,
                            enum_text(&a.kind)?,
                            a.name,
                            role,
                            asset_type,
                            a.net_movement,
                            a.cost_centre.map(|c| c.0)
                        ],
                    )?;
                }
                Change::Update(_, a) => {
                    // The role changes only as tickets take up a category or switch off.
                    let role = a.role.as_ref().map(enum_text).transpose()?;
                    let asset_type = a.asset_type.as_ref().map(enum_text).transpose()?;
                    conn.execute(
                        "UPDATE account SET name = ?2, role = ?3, asset_type = ?4, net_movement = ?5, cost_centre_id = ?6
                         WHERE id = ?1",
                        params![id, a.name, role, asset_type, a.net_movement, a.cost_centre.map(|c| c.0)],
                    )?;
                }
                Change::Delete(_) => {
                    conn.execute("DELETE FROM account WHERE id = ?1", [id])?;
                }
            }
            Ok(())
        },
    )?;

    saved(
        log,
        "person",
        "DELETE FROM person WHERE account_id = ?1",
        before,
        after,
        |b| b.persons().map(|(a, p)| (a.id.0, p)),
        |id, p| {
            conn.execute(
                "INSERT OR REPLACE INTO person (account_id, holder, iban, bic, other_method, contact, notes)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    p.holder,
                    p.iban.as_ref().map(|i| i.as_str()),
                    p.bic,
                    p.other_method,
                    p.contact,
                    p.notes
                ],
            )?;
            Ok(())
        },
    )?;

    // The event row, which also holds the ticket settings.
    let event = after.event();
    let row = |b: &'a Book| (b.event(), b.ticket_settings());
    if before.map(row) != Some(row(after)) {
        let d = &event.details;
        let tickets = after.ticket_settings();
        conn.execute(
            "INSERT INTO event (id, name, starts_on, ends_on, location, notes, closed_at,
                                default_reimbursement_account_id, closing_overrides,
                                tickets_enabled, ticket_account_id, currency)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT (id) DO UPDATE SET
               name = excluded.name, starts_on = excluded.starts_on, ends_on = excluded.ends_on,
               location = excluded.location, notes = excluded.notes, closed_at = excluded.closed_at,
               default_reimbursement_account_id = excluded.default_reimbursement_account_id,
               closing_overrides = excluded.closing_overrides,
               tickets_enabled = excluded.tickets_enabled,
               ticket_account_id = excluded.ticket_account_id,
               currency = excluded.currency",
            params![
                d.name,
                d.starts_on.map(date_text),
                d.ends_on.map(date_text),
                d.location,
                d.notes,
                event.closed_at.map(timestamp_text).transpose()?,
                event.default_reimbursement_account.0,
                serde_json::to_string(&event.closing_overrides)?,
                tickets.enabled,
                tickets.account.map(|a| a.0),
                enum_text(&d.currency)?,
            ],
        )?;
        match before.map(|b| b.event()) {
            None => log.write("event", 1, &Change::Create(event))?,
            Some(b) if b.is_closed() != event.is_closed() => {
                let json = |e| serde_json::to_string(e);
                let op = if event.is_closed() { "close" } else { "reopen" };
                log.raw("event", 1, op, Some(json(b)?), Some(json(event)?))?;
            }
            Some(b) if b != event => log.write("event", 1, &Change::Update(b, event))?,
            Some(_) => {}
        }
        setting(
            log,
            "ticket_settings",
            before.map(Book::ticket_settings).as_ref(),
            &tickets,
        )?;
    }

    saved(
        log,
        "assumption",
        "DELETE FROM assumption WHERE id = ?1",
        before,
        after,
        |b| b.assumptions().map(|a| (a.id.0, a)),
        |id, a| {
            conn.execute(
                "INSERT INTO assumption (id, name, value) VALUES (?1, ?2, ?3)
                 ON CONFLICT (id) DO UPDATE SET name = excluded.name, value = excluded.value",
                params![id, a.name, a.value],
            )?;
            Ok(())
        },
    )?;

    saved(
        log,
        "budget_line",
        "DELETE FROM budget_line WHERE id = ?1",
        before,
        after,
        |b| b.budget_lines().map(|l| (l.id.0, l)),
        |id, b| {
            let d = &b.draft;
            let (amount, quantity, unit_price, per) = match &d.estimate {
                Estimate::Amount { amount } => (Some(amount.get()), None, None, &[][..]),
                Estimate::Quantity {
                    quantity,
                    unit_price,
                    per,
                } => (
                    None,
                    Some(i64::from(*quantity)),
                    Some(unit_price.get()),
                    &per[..],
                ),
            };
            conn.execute(
                        "INSERT INTO budget_line (id, category_id, description, amount, quantity, unit_price, complete, cost_centre_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT (id) DO UPDATE SET
                           category_id = excluded.category_id, description = excluded.description,
                           amount = excluded.amount, quantity = excluded.quantity,
                           unit_price = excluded.unit_price, complete = excluded.complete,
                           cost_centre_id = excluded.cost_centre_id",
                        params![
                            id,
                            d.category.0,
                            d.description,
                            amount,
                            quantity,
                            unit_price,
                            d.complete,
                            d.cost_centre.map(|c| c.0)
                        ],
                    )?;
            conn.execute(
                "DELETE FROM budget_line_assumption WHERE budget_line_id = ?1",
                [id],
            )?;
            for (position, assumption) in per.iter().enumerate() {
                conn.execute(
                    "INSERT INTO budget_line_assumption (budget_line_id, position, assumption_id)
                     VALUES (?1, ?2, ?3)",
                    params![id, position as i64, assumption.0],
                )?;
            }
            Ok(())
        },
    )?;

    saved(
        log,
        "plugin_record",
        "DELETE FROM plugin_record WHERE id = ?1",
        before,
        after,
        |b| b.plugin_records().map(|r| (r.id.0, r)),
        |id, r| {
            conn.execute(
                "INSERT INTO plugin_record (id, plugin, key, scope, value)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (id) DO UPDATE SET
                   plugin = excluded.plugin, key = excluded.key, scope = excluded.scope,
                   value = excluded.value",
                params![
                    id,
                    r.plugin,
                    r.key,
                    enum_text(&r.scope)?,
                    serde_json::to_string(&r.value)?
                ],
            )?;
            Ok(())
        },
    )?;

    saved(
        log,
        "ticket_tier",
        "DELETE FROM ticket_tier WHERE id = ?1",
        before,
        after,
        |b| b.tiers().map(|t| (t.id.0, t)),
        |id, t| {
            let d = &t.draft;
            conn.execute(
                        "INSERT INTO ticket_tier (id, name, price, fee_bp, quota) VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT (id) DO UPDATE SET
                           name = excluded.name, price = excluded.price, fee_bp = excluded.fee_bp,
                           quota = excluded.quota",
                        params![id, d.name, d.price.get(), d.fee_bp, d.quota],
                    )?;
            conn.execute("DELETE FROM ticket_tier_alias WHERE tier_id = ?1", [id])?;
            for (position, alias) in d.aliases.iter().enumerate() {
                conn.execute(
                    "INSERT INTO ticket_tier_alias (tier_id, position, alias) VALUES (?1, ?2, ?3)",
                    params![id, position as i64, alias],
                )?;
            }
            Ok(())
        },
    )?;

    table(
        log,
        "ticket_import",
        before,
        after,
        |b| b.ticket_imports().map(|i| (i.id.0, i)),
        |id, change| {
            match change {
                Change::Create(i) => {
                    conn.execute(
                        "INSERT INTO ticket_import (id, filename, sha256, imported_at, date)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            id,
                            i.filename,
                            i.sha256,
                            timestamp_text(i.imported_at)?,
                            date_text(i.date)
                        ],
                    )?;
                    let mut stmt = conn.prepare(
                        "INSERT INTO ticket_import_row (import_id, order_id, tier_id, count) VALUES (?1, ?2, ?3, ?4)",
                    )?;
                    for o in &i.orders {
                        stmt.execute(params![id, o.order, o.tier.0, o.count])?;
                    }
                }
                // Only the date of an import changes; its rows are fixed.
                Change::Update(_, i) => {
                    conn.execute(
                        "UPDATE ticket_import SET date = ?2 WHERE id = ?1",
                        params![id, date_text(i.date)],
                    )?;
                }
                // Its rows go with it.
                Change::Delete(_) => {
                    conn.execute("DELETE FROM ticket_import WHERE id = ?1", [id])?;
                }
            }
            Ok(())
        },
    )?;

    saved(
        log,
        "ticket_manual_sale",
        "DELETE FROM ticket_manual_sale WHERE id = ?1",
        before,
        after,
        |b| b.manual_sales().map(|s| (s.id.0, s)),
        |id, s| {
            let d = &s.draft;
            conn.execute(
                        "INSERT INTO ticket_manual_sale (id, tier_id, count, destination_id, date, reference)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                         ON CONFLICT (id) DO UPDATE SET
                           tier_id = excluded.tier_id, count = excluded.count, destination_id = excluded.destination_id,
                           date = excluded.date, reference = excluded.reference",
                        params![id, d.tier.0, d.count, d.destination.0, date_text(d.date), d.reference],
                    )?;
            Ok(())
        },
    )?;

    saved(
        log,
        "ticket_cancellation",
        "DELETE FROM ticket_cancellation WHERE id = ?1",
        before,
        after,
        |b| b.cancellations().map(|c| (c.id.0, c)),
        |id, c| {
            let terms = c.terms.as_ref();
            conn.execute(
                        "INSERT INTO ticket_cancellation
                           (id, tier_id, count, order_id, sale_id, gone, note, refund_bp, date, payer_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                         ON CONFLICT (id) DO UPDATE SET
                           count = excluded.count, gone = excluded.gone, note = excluded.note,
                           refund_bp = excluded.refund_bp, date = excluded.date, payer_id = excluded.payer_id",
                        params![
                            id,
                            c.tier.0,
                            c.count,
                            c.order,
                            c.sale.map(|s| s.0),
                            c.gone,
                            c.note,
                            terms.map(|r| r.refund_bp),
                            terms.map(|r| date_text(r.date)),
                            terms.and_then(|r| r.payer).map(|a| a.0)
                        ],
                    )?;
            Ok(())
        },
    )?;

    // Transactions, with their lines replaced. A write is read back before it is kept.
    table(
        log,
        "txn",
        before,
        after,
        |b| b.txns().map(|t| (t.id.0, t)),
        |id, change| {
            match change {
                Change::Create(t) => {
                    let (origin, origin_id) = origin_text(t.origin);
                    conn.execute(
                        "INSERT INTO txn (id, kind, date, description, counterparty, notes, origin, origin_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            id,
                            enum_text(&t.kind)?,
                            date_text(t.date),
                            t.description,
                            t.counterparty,
                            t.notes,
                            origin,
                            origin_id
                        ],
                    )?;
                    write_lines(conn, t)?;
                }
                Change::Update(_, t) => {
                    conn.execute(
                        "UPDATE txn SET kind = ?2, date = ?3, description = ?4, counterparty = ?5, notes = ?6
                         WHERE id = ?1",
                        params![id, enum_text(&t.kind)?, date_text(t.date), t.description, t.counterparty, t.notes],
                    )?;
                    conn.execute("DELETE FROM line WHERE txn_id = ?1", [id])?;
                    write_lines(conn, t)?;
                }
                // Its lines and attachments go with it.
                Change::Delete(_) => {
                    conn.execute("DELETE FROM txn WHERE id = ?1", [id])?;
                }
            }
            Ok(())
        },
    )?;

    table(
        log,
        "attachment",
        before,
        after,
        |b| b.attachments().map(|a| (a.id.0, a)),
        |id, change| {
            match change {
                Change::Create(a) => {
                    let data = *blobs
                        .get(&AttachmentId(id))
                        .ok_or_else(|| StoreError::corrupt("attachment added without its data"))?;
                    conn.execute(
                        "INSERT INTO attachment (id, txn_id, filename, mime, size, sha256, added_at, data)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            id,
                            a.txn.0,
                            a.filename,
                            a.mime,
                            i64::try_from(a.size)
                                .map_err(|_| StoreError::corrupt("attachment too large"))?,
                            a.sha256,
                            timestamp_text(a.added_at)?,
                            data
                        ],
                    )?;
                }
                Change::Update(..) => {
                    return Err(StoreError::corrupt("an attachment cannot change"));
                }
                Change::Delete(_) => {
                    conn.execute("DELETE FROM attachment WHERE id = ?1", [id])?;
                }
            }
            Ok(())
        },
    )?;

    saved(
        log,
        "closing_count",
        "DELETE FROM closing_count WHERE account_id = ?1",
        before,
        after,
        |b| b.closing_counts().map(|c| (c.account.0, c)),
        |id, c| {
            conn.execute(
                        "INSERT OR REPLACE INTO closing_count (account_id, date, amount) VALUES (?1, ?2, ?3)",
                        params![id, date_text(c.date), c.amount.get()],
                    )?;
            Ok(())
        },
    )?;

    // Deletions and moves first, so a checkpoint can take the date another one just left.
    let checkpoints = |b: &'a Book| b.reconciliations().map(|r| (r.id.0, r));
    let changes = diff(
        before.map(checkpoints).into_iter().flatten(),
        checkpoints(after),
    );
    for (id, _) in changes
        .iter()
        .filter(|(_, c)| !matches!(c, Change::Create(_)))
    {
        conn.execute("DELETE FROM reconciliation WHERE id = ?1", [id])?;
    }
    for (id, change) in &changes {
        if let Change::Create(r) | Change::Update(_, r) = change {
            conn.execute(
                "INSERT INTO reconciliation (id, account_id, date, amount) VALUES (?1, ?2, ?3, ?4)",
                params![id, r.account.0, date_text(r.date), r.amount.get()],
            )?;
        }
        log.write("reconciliation", *id, change)?;
    }

    let next = after.next_ids();
    if before.map(|b| b.next_ids()) != Some(next) {
        for (key, value) in [
            ("next_account_id", next.account),
            ("next_txn_id", next.txn),
            ("next_attachment_id", next.attachment),
            ("next_budget_line_id", next.budget_line),
            ("next_assumption_id", next.assumption),
            ("next_tier_id", next.tier),
            ("next_import_id", next.import),
            ("next_manual_sale_id", next.manual_sale),
            ("next_cancellation_id", next.cancellation),
            ("next_cost_centre_id", next.cost_centre),
            ("next_plugin_record_id", next.plugin_record),
            ("next_reconciliation_id", next.reconciliation),
        ] {
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![key, value.to_string()],
            )?;
        }
    }
    Ok(())
}

/// Logs one of the settings stored on the event row, when it changed.
fn setting<V: Serialize + PartialEq>(
    log: &Log<'_>,
    entity: &str,
    before: Option<&V>,
    after: &V,
) -> Result<(), StoreError> {
    match before {
        None => log.write(entity, 1, &Change::Create(after)),
        Some(b) if b != after => log.write(entity, 1, &Change::Update(b, after)),
        Some(_) => Ok(()),
    }
}

fn write_lines(conn: &Connection, t: &Txn) -> Result<(), StoreError> {
    for (position, l) in t.lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO line (txn_id, position, account_id, amount, budget_line_id, cost_centre_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                t.id.0,
                position as i64,
                l.account.0,
                l.amount.get(),
                l.budget_line.map(|b| b.0),
                l.cost_centre.map(|c| c.0)
            ],
        )?;
    }
    Ok(())
}

/// A transaction's origin as stored: its kind and the tier, sale or cancellation it comes from.
fn origin_text(origin: Origin) -> (&'static str, Option<i64>) {
    match origin {
        Origin::Manual => ("manual", None),
        Origin::TicketImport { tier } => ("ticket_import", Some(tier.0)),
        Origin::TicketSale { sale } => ("ticket_sale", Some(sale.0)),
        Origin::TicketRefund { cancellation } => ("ticket_refund", Some(cancellation.0)),
    }
}
