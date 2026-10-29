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
use crate::model::{
    Account, AccountId, Attachment, AttachmentId, BudgetLine, BudgetLineId, ClosingCount, Estimate,
    Person, Txn, TxnId,
};

enum Change<'a, V> {
    Create(&'a V),
    Update(&'a V, &'a V),
    Delete(&'a V),
}

fn diff<'a, K: Ord + Copy, V: PartialEq>(
    before: impl IntoIterator<Item = (K, &'a V)>,
    after: impl IntoIterator<Item = (K, &'a V)>,
) -> Vec<(K, Change<'a, V>)> {
    let before: BTreeMap<K, &V> = before.into_iter().collect();
    let after: BTreeMap<K, &V> = after.into_iter().collect();
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

/// Writes everything in `after` that differs from `before` (`None`: an empty file).
/// `blobs` holds the bytes of the attachments the command added, by their new ids.
pub(crate) fn persist(
    conn: &Connection,
    before: Option<&Book>,
    after: &Book,
    blobs: &BTreeMap<AttachmentId, &[u8]>,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    let log = Log {
        conn,
        at: timestamp_text(now)?,
    };

    // Accounts and persons first: everything else refers to them.
    fn accounts(b: Option<&Book>) -> Vec<(AccountId, &Account)> {
        b.map(|b| b.accounts().map(|a| (a.id, a)).collect())
            .unwrap_or_default()
    }
    for (id, change) in diff(accounts(before), accounts(Some(after))) {
        match &change {
            Change::Create(a) => {
                let role = a.role.as_ref().map(enum_text).transpose()?;
                let asset_type = a.asset_type.as_ref().map(enum_text).transpose()?;
                conn.execute(
                    "INSERT INTO account (id, kind, name, role, asset_type) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id.0, enum_text(&a.kind)?, a.name, role, asset_type],
                )?;
            }
            Change::Update(_, a) => {
                let asset_type = a.asset_type.as_ref().map(enum_text).transpose()?;
                conn.execute(
                    "UPDATE account SET name = ?2, asset_type = ?3 WHERE id = ?1",
                    params![id.0, a.name, asset_type],
                )?;
            }
            Change::Delete(_) => {
                conn.execute("DELETE FROM account WHERE id = ?1", [id.0])?;
            }
        }
        log.write("account", id.0, &change)?;
    }

    fn persons(b: Option<&Book>) -> Vec<(AccountId, &Person)> {
        b.map(|b| b.persons().map(|(a, p)| (a.id, p)).collect())
            .unwrap_or_default()
    }
    for (id, change) in diff(persons(before), persons(Some(after))) {
        match &change {
            Change::Create(p) | Change::Update(_, p) => {
                conn.execute(
                    "INSERT OR REPLACE INTO person (account_id, holder, iban, bic, contact, notes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        id.0,
                        p.holder,
                        p.iban.as_ref().map(|i| i.as_str()),
                        p.bic,
                        p.contact,
                        p.notes
                    ],
                )?;
            }
            Change::Delete(_) => {
                conn.execute("DELETE FROM person WHERE account_id = ?1", [id.0])?;
            }
        }
        log.write("person", id.0, &change)?;
    }

    // The event row.
    let event = after.event();
    if before.map(|b| b.event()) != Some(event) {
        let d = &event.details;
        conn.execute(
            "INSERT INTO event (id, name, starts_on, ends_on, location, notes, closed_at,
                                default_reimbursement_account_id, closing_overrides)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (id) DO UPDATE SET
               name = excluded.name, starts_on = excluded.starts_on, ends_on = excluded.ends_on,
               location = excluded.location, notes = excluded.notes, closed_at = excluded.closed_at,
               default_reimbursement_account_id = excluded.default_reimbursement_account_id,
               closing_overrides = excluded.closing_overrides",
            params![
                d.name,
                d.starts_on.map(date_text),
                d.ends_on.map(date_text),
                d.location,
                d.notes,
                event.closed_at.map(timestamp_text).transpose()?,
                event.default_reimbursement_account.0,
                serde_json::to_string(&event.closing_overrides)?,
            ],
        )?;
        match before.map(|b| b.event()) {
            None => log.write("event", 1, &Change::Create(event))?,
            Some(b) if b.is_closed() != event.is_closed() => {
                let json = |e| serde_json::to_string(e);
                let op = if event.is_closed() { "close" } else { "reopen" };
                log.raw("event", 1, op, Some(json(b)?), Some(json(event)?))?;
            }
            Some(b) => log.write("event", 1, &Change::Update(b, event))?,
        }
    }

    // Attachments that went away, before their transactions do.
    fn attachments(b: Option<&Book>) -> Vec<(AttachmentId, &Attachment)> {
        b.map(|b| b.attachments().map(|a| (a.id, a)).collect())
            .unwrap_or_default()
    }
    let attachment_changes = diff(attachments(before), attachments(Some(after)));
    for (id, change) in &attachment_changes {
        if let Change::Delete(_) = change {
            conn.execute("DELETE FROM attachment WHERE id = ?1", [id.0])?;
            log.write("attachment", id.0, change)?;
        }
    }

    // Budget lines before the transactions that link to them; deleted ones after.
    fn budget_lines(b: Option<&Book>) -> Vec<(BudgetLineId, &BudgetLine)> {
        b.map(|b| b.budget_lines().map(|l| (l.id, l)).collect())
            .unwrap_or_default()
    }
    let budget_changes = diff(budget_lines(before), budget_lines(Some(after)));
    for (id, change) in &budget_changes {
        match change {
            Change::Create(b) | Change::Update(_, b) => {
                let d = &b.draft;
                let (amount, quantity, unit_price) = match d.estimate {
                    Estimate::Amount { amount } => (Some(amount.get()), None, None),
                    Estimate::Quantity {
                        quantity,
                        unit_price,
                    } => (None, Some(i64::from(quantity)), Some(unit_price.get())),
                };
                conn.execute(
                    "INSERT INTO budget_line (id, category_id, description, amount, quantity, unit_price, complete)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT (id) DO UPDATE SET
                       category_id = excluded.category_id, description = excluded.description,
                       amount = excluded.amount, quantity = excluded.quantity,
                       unit_price = excluded.unit_price, complete = excluded.complete",
                    params![id.0, d.category.0, d.description, amount, quantity, unit_price, d.complete],
                )?;
                log.write("budget_line", id.0, change)?;
            }
            Change::Delete(_) => {}
        }
    }

    // Transactions: unseal, replace the lines, seal (R-COR-2).
    fn txns(b: Option<&Book>) -> Vec<(TxnId, &Txn)> {
        b.map(|b| b.txns().map(|t| (t.id, t)).collect())
            .unwrap_or_default()
    }
    for (id, change) in diff(txns(before), txns(Some(after))) {
        match &change {
            Change::Create(t) => {
                conn.execute(
                    "INSERT INTO txn (id, kind, date, description, counterparty, notes, sealed)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
                    params![
                        id.0,
                        enum_text(&t.kind)?,
                        date_text(t.date),
                        t.description,
                        t.counterparty,
                        t.notes
                    ],
                )?;
                write_lines(conn, t)?;
            }
            Change::Update(_, t) => {
                conn.execute(
                    "UPDATE txn SET kind = ?2, date = ?3, description = ?4, counterparty = ?5, notes = ?6, sealed = 0
                     WHERE id = ?1",
                    params![id.0, enum_text(&t.kind)?, date_text(t.date), t.description, t.counterparty, t.notes],
                )?;
                conn.execute("DELETE FROM line WHERE txn_id = ?1", [id.0])?;
                write_lines(conn, t)?;
            }
            Change::Delete(_) => {
                conn.execute("UPDATE txn SET sealed = 0 WHERE id = ?1", [id.0])?;
                conn.execute("DELETE FROM txn WHERE id = ?1", [id.0])?;
            }
        }
        log.write("txn", id.0, &change)?;
    }

    for (id, change) in &budget_changes {
        if let Change::Delete(_) = change {
            conn.execute("DELETE FROM budget_line WHERE id = ?1", [id.0])?;
            log.write("budget_line", id.0, change)?;
        }
    }

    // New attachments, with their bytes.
    for (id, change) in &attachment_changes {
        if let Change::Create(a) = change {
            let data = *blobs
                .get(id)
                .ok_or_else(|| StoreError::corrupt("attachment added without its data"))?;
            conn.execute(
                "INSERT INTO attachment (id, txn_id, filename, mime, size, sha256, added_at, data)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id.0,
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
            log.write("attachment", id.0, change)?;
        }
    }

    fn counts(b: Option<&Book>) -> Vec<(AccountId, &ClosingCount)> {
        b.map(|b| b.closing_counts().map(|c| (c.account, c)).collect())
            .unwrap_or_default()
    }
    for (id, change) in diff(counts(before), counts(Some(after))) {
        match &change {
            Change::Create(c) | Change::Update(_, c) => {
                conn.execute(
                    "INSERT OR REPLACE INTO closing_count (account_id, date, amount) VALUES (?1, ?2, ?3)",
                    params![id.0, date_text(c.date), c.amount.get()],
                )?;
            }
            Change::Delete(_) => {
                conn.execute("DELETE FROM closing_count WHERE account_id = ?1", [id.0])?;
            }
        }
        log.write("closing_count", id.0, &change)?;
    }

    let next = after.next_ids();
    if before.map(|b| b.next_ids()) != Some(next) {
        for (key, value) in [
            ("next_account_id", next.account),
            ("next_txn_id", next.txn),
            ("next_attachment_id", next.attachment),
            ("next_budget_line_id", next.budget_line),
        ] {
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![key, value.to_string()],
            )?;
        }
    }
    Ok(())
}

fn write_lines(conn: &Connection, t: &Txn) -> Result<(), StoreError> {
    for (position, l) in t.lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO line (txn_id, position, account_id, amount, memo, budget_line_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                t.id.0,
                position as i64,
                l.account.0,
                l.amount.get(),
                l.memo,
                l.budget_line.map(|b| b.0)
            ],
        )?;
    }
    conn.execute("UPDATE txn SET sealed = 1 WHERE id = ?1", [t.id.0])?;
    Ok(())
}
