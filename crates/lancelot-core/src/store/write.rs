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
use crate::drinks::DrinkProduct;
use crate::model::{
    Account, AccountId, Attachment, AttachmentId, BudgetLine, BudgetLineId, CancellationId,
    ClosingCount, CostCentre, CostCentreId, DrinkProductId, Estimate, ImportId, ManualSaleId,
    Origin, Person, Reconciliation, ReconciliationId, TierId, Txn, TxnId,
};
use crate::tickets::{Cancellation, ManualSale, TicketImport, Tier};

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

    // Cost centres first: categories, budget lines and transaction lines refer to them; deleted
    // ones go last.
    fn cost_centres(b: Option<&Book>) -> Vec<(CostCentreId, &CostCentre)> {
        b.map(|b| b.cost_centres().map(|c| (c.id, c)).collect())
            .unwrap_or_default()
    }
    let centre_changes = diff(cost_centres(before), cost_centres(Some(after)));
    for (id, change) in &centre_changes {
        if let Change::Create(c) | Change::Update(_, c) = change {
            let role = c.role.as_ref().map(enum_text).transpose()?;
            conn.execute(
                "INSERT INTO cost_centre (id, name, role) VALUES (?1, ?2, ?3)
                 ON CONFLICT (id) DO UPDATE SET name = excluded.name, role = excluded.role",
                params![id.0, c.name, role],
            )?;
            log.write("cost_centre", id.0, change)?;
        }
    }

    // Accounts and persons next: everything else refers to them.
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
                    "INSERT INTO account (id, kind, name, role, asset_type, cost_centre_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        id.0,
                        enum_text(&a.kind)?,
                        a.name,
                        role,
                        asset_type,
                        a.cost_centre.map(|c| c.0)
                    ],
                )?;
            }
            Change::Update(_, a) => {
                // The role changes only when a category becomes the ticket fee category.
                let role = a.role.as_ref().map(enum_text).transpose()?;
                let asset_type = a.asset_type.as_ref().map(enum_text).transpose()?;
                conn.execute(
                    "UPDATE account SET name = ?2, role = ?3, asset_type = ?4, cost_centre_id = ?5
                     WHERE id = ?1",
                    params![id.0, a.name, role, asset_type, a.cost_centre.map(|c| c.0)],
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
        let columns = &event.ticket_columns;
        conn.execute(
            "INSERT INTO event (id, name, starts_on, ends_on, location, notes, closed_at,
                                default_reimbursement_account_id, closing_overrides,
                                ticket_tier_column, ticket_order_column, ticket_date_column,
                                drink_supplier, price_per_strich)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT (id) DO UPDATE SET
               name = excluded.name, starts_on = excluded.starts_on, ends_on = excluded.ends_on,
               location = excluded.location, notes = excluded.notes, closed_at = excluded.closed_at,
               default_reimbursement_account_id = excluded.default_reimbursement_account_id,
               closing_overrides = excluded.closing_overrides,
               ticket_tier_column = excluded.ticket_tier_column,
               ticket_order_column = excluded.ticket_order_column,
               ticket_date_column = excluded.ticket_date_column,
               drink_supplier = excluded.drink_supplier,
               price_per_strich = excluded.price_per_strich",
            params![
                d.name,
                d.starts_on.map(date_text),
                d.ends_on.map(date_text),
                d.location,
                d.notes,
                event.closed_at.map(timestamp_text).transpose()?,
                event.default_reimbursement_account.0,
                serde_json::to_string(&event.closing_overrides)?,
                columns.tier,
                columns.order,
                columns.sale_date,
                event.drinks.supplier,
                event.drinks.price_per_strich.map(|p| p.get()),
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
                    "INSERT INTO budget_line (id, category_id, description, amount, quantity, unit_price, complete, cost_centre_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT (id) DO UPDATE SET
                       category_id = excluded.category_id, description = excluded.description,
                       amount = excluded.amount, quantity = excluded.quantity,
                       unit_price = excluded.unit_price, complete = excluded.complete,
                       cost_centre_id = excluded.cost_centre_id",
                    params![
                        id.0,
                        d.category.0,
                        d.description,
                        amount,
                        quantity,
                        unit_price,
                        d.complete,
                        d.cost_centre.map(|c| c.0)
                    ],
                )?;
                log.write("budget_line", id.0, change)?;
            }
            Change::Delete(_) => {}
        }
    }

    fn products(b: Option<&Book>) -> Vec<(DrinkProductId, &DrinkProduct)> {
        b.map(|b| b.drink_products().map(|p| (p.id, p)).collect())
            .unwrap_or_default()
    }
    for (id, change) in diff(products(before), products(Some(after))) {
        match &change {
            Change::Create(p) | Change::Update(_, p) => {
                let d = &p.draft;
                conn.execute(
                    "INSERT INTO drink_product (id, name, taken, returned, unit_price)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT (id) DO UPDATE SET
                       name = excluded.name, taken = excluded.taken, returned = excluded.returned,
                       unit_price = excluded.unit_price",
                    params![id.0, d.name, d.taken, d.returned, d.unit_price.get()],
                )?;
            }
            Change::Delete(_) => {
                conn.execute("DELETE FROM drink_product WHERE id = ?1", [id.0])?;
            }
        }
        log.write("drink_product", id.0, &change)?;
    }

    // Tickets before the transactions generated from them; deleted ones after.
    fn tiers(b: Option<&Book>) -> Vec<(TierId, &Tier)> {
        b.map(|b| b.tiers().map(|t| (t.id, t)).collect())
            .unwrap_or_default()
    }
    let tier_changes = diff(tiers(before), tiers(Some(after)));
    for (id, change) in &tier_changes {
        if let Change::Create(t) | Change::Update(_, t) = change {
            let d = &t.draft;
            conn.execute(
                "INSERT INTO ticket_tier (id, name, price, fee_bp, quota) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (id) DO UPDATE SET
                   name = excluded.name, price = excluded.price, fee_bp = excluded.fee_bp,
                   quota = excluded.quota",
                params![id.0, d.name, d.price.get(), d.fee_bp, d.quota],
            )?;
            conn.execute("DELETE FROM ticket_tier_alias WHERE tier_id = ?1", [id.0])?;
            for (position, alias) in d.aliases.iter().enumerate() {
                conn.execute(
                    "INSERT INTO ticket_tier_alias (tier_id, position, alias) VALUES (?1, ?2, ?3)",
                    params![id.0, position as i64, alias],
                )?;
            }
            log.write("ticket_tier", id.0, change)?;
        }
    }

    fn imports(b: Option<&Book>) -> Vec<(ImportId, &TicketImport)> {
        b.map(|b| b.ticket_imports().map(|i| (i.id, i)).collect())
            .unwrap_or_default()
    }
    let import_changes = diff(imports(before), imports(Some(after)));
    for (id, change) in &import_changes {
        match change {
            Change::Create(i) => {
                conn.execute(
                    "INSERT INTO ticket_import (id, filename, sha256, imported_at, date)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        id.0,
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
                    stmt.execute(params![id.0, o.order, o.tier.0, o.count])?;
                }
            }
            // Only the date of an import changes; its rows are fixed.
            Change::Update(_, i) => {
                conn.execute(
                    "UPDATE ticket_import SET date = ?2 WHERE id = ?1",
                    params![id.0, date_text(i.date)],
                )?;
            }
            // Its rows go with it.
            Change::Delete(_) => {
                conn.execute("DELETE FROM ticket_import WHERE id = ?1", [id.0])?;
            }
        }
        log.write("ticket_import", id.0, change)?;
    }

    fn sales(b: Option<&Book>) -> Vec<(ManualSaleId, &ManualSale)> {
        b.map(|b| b.manual_sales().map(|s| (s.id, s)).collect())
            .unwrap_or_default()
    }
    let sale_changes = diff(sales(before), sales(Some(after)));
    for (id, change) in &sale_changes {
        if let Change::Create(s) | Change::Update(_, s) = change {
            let d = &s.draft;
            conn.execute(
                "INSERT INTO ticket_manual_sale (id, tier_id, count, person_id, date, reference)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (id) DO UPDATE SET
                   tier_id = excluded.tier_id, count = excluded.count, person_id = excluded.person_id,
                   date = excluded.date, reference = excluded.reference",
                params![id.0, d.tier.0, d.count, d.person.0, date_text(d.date), d.reference],
            )?;
            log.write("ticket_manual_sale", id.0, change)?;
        }
    }

    fn cancellations(b: Option<&Book>) -> Vec<(CancellationId, &Cancellation)> {
        b.map(|b| b.cancellations().map(|c| (c.id, c)).collect())
            .unwrap_or_default()
    }
    let cancellation_changes = diff(cancellations(before), cancellations(Some(after)));
    for (id, change) in &cancellation_changes {
        if let Change::Create(c) | Change::Update(_, c) = change {
            let terms = c.terms.as_ref();
            conn.execute(
                "INSERT INTO ticket_cancellation
                   (id, tier_id, count, order_id, sale_id, gone, note, refund_bp, date, payer_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT (id) DO UPDATE SET
                   count = excluded.count, gone = excluded.gone, note = excluded.note,
                   refund_bp = excluded.refund_bp, date = excluded.date, payer_id = excluded.payer_id",
                params![
                    id.0,
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
        }
    }
    // Before any sale it refers to goes.
    for (id, change) in &cancellation_changes {
        if let Change::Delete(_) = change {
            conn.execute("DELETE FROM ticket_cancellation WHERE id = ?1", [id.0])?;
        }
        log.write("ticket_cancellation", id.0, change)?;
    }

    // Transactions: unseal, replace the lines, seal (R-COR-2).
    fn txns(b: Option<&Book>) -> Vec<(TxnId, &Txn)> {
        b.map(|b| b.txns().map(|t| (t.id, t)).collect())
            .unwrap_or_default()
    }
    for (id, change) in diff(txns(before), txns(Some(after))) {
        match &change {
            Change::Create(t) => {
                let (origin, origin_id) = origin_text(t.origin);
                conn.execute(
                    "INSERT INTO txn (id, kind, date, description, counterparty, notes, origin, origin_id, sealed)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
                    params![
                        id.0,
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
    for (id, change) in &sale_changes {
        if let Change::Delete(_) = change {
            conn.execute("DELETE FROM ticket_manual_sale WHERE id = ?1", [id.0])?;
            log.write("ticket_manual_sale", id.0, change)?;
        }
    }
    for (id, change) in &import_changes {
        if let Change::Delete(_) = change {
            conn.execute("DELETE FROM ticket_import WHERE id = ?1", [id.0])?;
            log.write("ticket_import", id.0, change)?;
        }
    }
    for (id, change) in &tier_changes {
        if let Change::Delete(_) = change {
            conn.execute("DELETE FROM ticket_tier WHERE id = ?1", [id.0])?;
            log.write("ticket_tier", id.0, change)?;
        }
    }

    for (id, change) in &centre_changes {
        if let Change::Delete(_) = change {
            conn.execute("DELETE FROM cost_centre WHERE id = ?1", [id.0])?;
            log.write("cost_centre", id.0, change)?;
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

    fn checkpoints(b: Option<&Book>) -> Vec<(ReconciliationId, &Reconciliation)> {
        b.map(|b| b.reconciliations().map(|r| (r.id, r)).collect())
            .unwrap_or_default()
    }
    // Deletions and moves first, so a checkpoint can take the date another one just left.
    let changes = diff(checkpoints(before), checkpoints(Some(after)));
    for (id, _) in changes
        .iter()
        .filter(|(_, c)| !matches!(c, Change::Create(_)))
    {
        conn.execute("DELETE FROM reconciliation WHERE id = ?1", [id.0])?;
    }
    for (id, change) in &changes {
        if let Change::Create(r) | Change::Update(_, r) = change {
            conn.execute(
                "INSERT INTO reconciliation (id, account_id, date, amount) VALUES (?1, ?2, ?3, ?4)",
                params![id.0, r.account.0, date_text(r.date), r.amount.get()],
            )?;
        }
        log.write("reconciliation", id.0, change)?;
    }

    let next = after.next_ids();
    if before.map(|b| b.next_ids()) != Some(next) {
        for (key, value) in [
            ("next_account_id", next.account),
            ("next_txn_id", next.txn),
            ("next_attachment_id", next.attachment),
            ("next_budget_line_id", next.budget_line),
            ("next_tier_id", next.tier),
            ("next_import_id", next.import),
            ("next_manual_sale_id", next.manual_sale),
            ("next_cancellation_id", next.cancellation),
            ("next_cost_centre_id", next.cost_centre),
            ("next_drink_product_id", next.drink_product),
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

fn write_lines(conn: &Connection, t: &Txn) -> Result<(), StoreError> {
    for (position, l) in t.lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO line (txn_id, position, account_id, amount, memo, budget_line_id, cost_centre_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                t.id.0,
                position as i64,
                l.account.0,
                l.amount.get(),
                l.memo,
                l.budget_line.map(|b| b.0),
                l.cost_centre.map(|c| c.0)
            ],
        )?;
    }
    conn.execute("UPDATE txn SET sealed = 1 WHERE id = ?1", [t.id.0])?;
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
