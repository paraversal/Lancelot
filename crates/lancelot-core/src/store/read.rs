//! Reads an event file into [`BookParts`].

use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension};

use super::StoreError;
use super::codec::{text_date, text_enum, text_timestamp};
use crate::book::{BookParts, NextIds};
use crate::iban::validate_iban;
use crate::model::{
    Account, AccountId, Attachment, AttachmentId, BudgetLine, BudgetLineDraft, BudgetLineId,
    CancellationId, ClosingCount, Estimate, Event, EventDetails, ImportId, Line, ManualSaleId,
    Origin, Person, TierId, Txn, TxnId,
};
use crate::money::Cents;
use crate::tickets::{
    Cancellation, ManualSale, ManualSaleDraft, OrderCount, RefundTerms, TicketColumns,
    TicketImport, Tier, TierDraft,
};

pub(crate) fn cents(value: i64) -> Result<Cents, StoreError> {
    Cents::new(value).map_err(|_| StoreError::corrupt(format!("amount out of range: {value}")))
}

pub(crate) fn meta(conn: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()?)
}

/// The event's UUID, which names its backup folder. Validated, so a damaged file cannot
/// point backups elsewhere.
pub(crate) fn event_uuid(conn: &Connection) -> Result<String, StoreError> {
    let text =
        meta(conn, "event_uuid")?.ok_or_else(|| StoreError::corrupt("missing event_uuid"))?;
    uuid::Uuid::parse_str(&text)
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| StoreError::corrupt("invalid event_uuid"))
}

fn next_id(conn: &Connection, key: &str) -> Result<i64, StoreError> {
    meta(conn, key)?
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| StoreError::corrupt(format!("missing {key}")))
}

pub(crate) fn accounts(conn: &Connection) -> Result<BTreeMap<AccountId, Account>, StoreError> {
    let mut stmt =
        conn.prepare("SELECT id, kind, name, role, asset_type FROM account ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (id, kind, name, role, asset_type) = row?;
        let id = AccountId(id);
        let role = role.as_deref().map(text_enum).transpose()?;
        let asset_type = asset_type.as_deref().map(text_enum).transpose()?;
        out.insert(
            id,
            Account {
                id,
                kind: text_enum(&kind)?,
                name,
                role,
                asset_type,
            },
        );
    }
    Ok(out)
}

/// All transactions with their lines, ordered by id and line position.
pub(crate) fn txns(conn: &Connection) -> Result<Vec<Txn>, StoreError> {
    let mut stmt = conn
        .prepare("SELECT id, kind, date, description, counterparty, notes, origin, origin_id FROM txn ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            (
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ),
            (r.get::<_, String>(6)?, r.get::<_, Option<i64>>(7)?),
        ))
    })?;
    let mut txns = BTreeMap::new();
    for row in rows {
        let ((id, kind, date, description, counterparty, notes), origin) = row?;
        let origin = match origin {
            (o, None) if o == "manual" => Origin::Manual,
            (o, Some(tier)) if o == "ticket_import" => Origin::TicketImport { tier: TierId(tier) },
            (o, Some(sale)) if o == "ticket_sale" => Origin::TicketSale {
                sale: ManualSaleId(sale),
            },
            (o, Some(cancellation)) if o == "ticket_refund" => Origin::TicketRefund {
                cancellation: CancellationId(cancellation),
            },
            (o, _) => {
                return Err(StoreError::corrupt(format!(
                    "transaction {id} has origin `{o}`"
                )));
            }
        };
        let id = TxnId(id);
        txns.insert(
            id,
            Txn {
                id,
                kind: text_enum(&kind)?,
                date: text_date(&date)?,
                description,
                counterparty,
                notes,
                lines: Vec::new(),
                origin,
            },
        );
    }

    let mut stmt = conn
        .prepare("SELECT txn_id, account_id, amount, memo, budget_line_id FROM line ORDER BY txn_id, position")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    })?;
    for row in rows {
        let (txn, account, amount, memo, budget_line) = row?;
        let txn = txns
            .get_mut(&TxnId(txn))
            .ok_or_else(|| StoreError::corrupt(format!("line of unknown transaction {txn}")))?;
        txn.lines.push(Line {
            account: AccountId(account),
            amount: cents(amount)?,
            memo,
            budget_line: budget_line.map(BudgetLineId),
        });
    }
    Ok(txns.into_values().collect())
}

pub(crate) fn parts(conn: &Connection) -> Result<BookParts, StoreError> {
    let event = conn.query_row(
        "SELECT name, starts_on, ends_on, location, notes, closed_at, default_reimbursement_account_id, closing_overrides,
                ticket_tier_column, ticket_order_column, ticket_date_column
         FROM event WHERE id = 1",
        [],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
                TicketColumns {
                    tier: r.get(8)?,
                    order: r.get(9)?,
                    sale_date: r.get(10)?,
                },
            ))
        },
    )?;
    let (name, starts_on, ends_on, location, notes, closed_at, default_account, overrides, columns) =
        event;
    let event = Event {
        details: EventDetails {
            name,
            starts_on: starts_on.as_deref().map(text_date).transpose()?,
            ends_on: ends_on.as_deref().map(text_date).transpose()?,
            location,
            notes,
        },
        closed_at: closed_at.as_deref().map(text_timestamp).transpose()?,
        default_reimbursement_account: AccountId(default_account),
        closing_overrides: serde_json::from_str(&overrides)?,
        ticket_columns: columns,
    };

    let mut stmt = conn.prepare(
        "SELECT account_id, holder, iban, bic, contact, notes FROM person ORDER BY account_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut persons = Vec::new();
    for row in rows {
        let (id, holder, iban, bic, contact, notes) = row?;
        let iban = iban
            .map(|i| {
                validate_iban(&i)
                    .map_err(|_| StoreError::corrupt(format!("invalid IBAN of person {id}")))
            })
            .transpose()?;
        persons.push((
            AccountId(id),
            Person {
                holder,
                iban,
                bic,
                contact,
                notes,
            },
        ));
    }

    let mut stmt = conn.prepare(
        "SELECT id, txn_id, filename, mime, size, sha256, added_at FROM attachment ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
        ))
    })?;
    let mut attachments = Vec::new();
    for row in rows {
        let (id, txn, filename, mime, size, sha256, added_at) = row?;
        attachments.push(Attachment {
            id: AttachmentId(id),
            txn: TxnId(txn),
            filename,
            mime,
            size: u64::try_from(size)
                .map_err(|_| StoreError::corrupt("negative attachment size"))?,
            sha256,
            added_at: text_timestamp(&added_at)?,
        });
    }

    let mut stmt =
        conn.prepare("SELECT account_id, date, amount FROM closing_count ORDER BY account_id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut closing_counts = Vec::new();
    for row in rows {
        let (account, date, amount) = row?;
        closing_counts.push(ClosingCount {
            account: AccountId(account),
            date: text_date(&date)?,
            amount: cents(amount)?,
        });
    }

    let mut stmt = conn.prepare(
        "SELECT id, category_id, description, amount, quantity, unit_price, complete
         FROM budget_line ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, Option<i64>>(5)?,
            r.get::<_, bool>(6)?,
        ))
    })?;
    let mut budget_lines = Vec::new();
    for row in rows {
        let (id, category, description, amount, quantity, unit_price, complete) = row?;
        let estimate = match (amount, quantity, unit_price) {
            (Some(amount), None, None) => Estimate::Amount {
                amount: cents(amount)?,
            },
            (None, Some(quantity), Some(unit_price)) => Estimate::Quantity {
                quantity: u32::try_from(quantity).map_err(|_| {
                    StoreError::corrupt(format!("quantity out of range: {quantity}"))
                })?,
                unit_price: cents(unit_price)?,
            },
            _ => {
                return Err(StoreError::corrupt(format!(
                    "budget line {id} has no estimate"
                )));
            }
        };
        budget_lines.push(BudgetLine {
            id: BudgetLineId(id),
            draft: BudgetLineDraft {
                category: AccountId(category),
                description,
                estimate,
                complete,
            },
        });
    }

    let (tiers, imports, manual_sales) = tickets(conn)?;
    let cancellations = cancellations(conn)?;

    Ok(BookParts {
        event,
        accounts: accounts(conn)?.into_values().collect(),
        persons,
        txns: txns(conn)?,
        attachments,
        closing_counts,
        budget_lines,
        tiers,
        imports,
        manual_sales,
        cancellations,
        next: NextIds {
            account: next_id(conn, "next_account_id")?,
            txn: next_id(conn, "next_txn_id")?,
            attachment: next_id(conn, "next_attachment_id")?,
            budget_line: next_id(conn, "next_budget_line_id")?,
            tier: next_id(conn, "next_tier_id")?,
            import: next_id(conn, "next_import_id")?,
            manual_sale: next_id(conn, "next_manual_sale_id")?,
            cancellation: next_id(conn, "next_cancellation_id")?,
        },
    })
}

fn count(value: i64) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| StoreError::corrupt(format!("count out of range: {value}")))
}

type TicketParts = (Vec<Tier>, Vec<TicketImport>, Vec<ManualSale>);

/// Ticket tiers with their aliases, imports with their rows, and manual sales.
fn tickets(conn: &Connection) -> Result<TicketParts, StoreError> {
    let mut aliases: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    let mut stmt =
        conn.prepare("SELECT tier_id, alias FROM ticket_tier_alias ORDER BY tier_id, position")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (tier, alias) = row?;
        aliases.entry(tier).or_default().push(alias);
    }
    let mut stmt =
        conn.prepare("SELECT id, name, price, fee_bp, quota FROM ticket_tier ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, Option<i64>>(4)?,
        ))
    })?;
    let mut tiers = Vec::new();
    for row in rows {
        let (id, name, price, fee_bp, quota) = row?;
        tiers.push(Tier {
            id: TierId(id),
            draft: TierDraft {
                name,
                price: cents(price)?,
                fee_bp: count(fee_bp)?,
                quota: quota.map(count).transpose()?,
                aliases: aliases.remove(&id).unwrap_or_default(),
            },
        });
    }

    let mut orders: BTreeMap<i64, Vec<OrderCount>> = BTreeMap::new();
    let mut stmt = conn.prepare(
        "SELECT import_id, order_id, tier_id, count FROM ticket_import_row
         ORDER BY import_id, order_id, tier_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (import, order, tier, n) = row?;
        orders.entry(import).or_default().push(OrderCount {
            order,
            tier: TierId(tier),
            count: count(n)?,
        });
    }
    let mut stmt = conn
        .prepare("SELECT id, filename, sha256, imported_at, date FROM ticket_import ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    let mut imports = Vec::new();
    for row in rows {
        let (id, filename, sha256, imported_at, date) = row?;
        let mut rows = orders.remove(&id).unwrap_or_default();
        // SQLite sorts text by bytes, like Rust, but keep the book's order regardless.
        rows.sort();
        imports.push(TicketImport {
            id: ImportId(id),
            filename,
            sha256,
            imported_at: text_timestamp(&imported_at)?,
            date: text_date(&date)?,
            orders: rows,
        });
    }

    let mut stmt = conn.prepare(
        "SELECT id, tier_id, count, person_id, date, reference FROM ticket_manual_sale ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut sales = Vec::new();
    for row in rows {
        let (id, tier, n, person, date, reference) = row?;
        sales.push(ManualSale {
            id: ManualSaleId(id),
            draft: ManualSaleDraft {
                tier: TierId(tier),
                count: count(n)?,
                person: AccountId(person),
                date: text_date(&date)?,
                reference,
            },
        });
    }
    Ok((tiers, imports, sales))
}

/// Cancellations and corrections: a correction has no refund terms.
fn cancellations(conn: &Connection) -> Result<Vec<Cancellation>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, tier_id, count, order_id, sale_id, gone, note, refund_bp, date, payer_id
         FROM ticket_cancellation ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            (
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
            ),
            (
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<i64>>(9)?,
            ),
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let ((id, tier, n, order, sale), (gone, note, refund_bp, date, payer)) = row?;
        let terms = match (refund_bp, date) {
            (Some(refund_bp), Some(date)) => Some(RefundTerms {
                refund_bp: u32::try_from(refund_bp)
                    .map_err(|_| StoreError::corrupt("refund out of range"))?,
                date: text_date(&date)?,
                payer: payer.map(AccountId),
            }),
            _ => None,
        };
        out.push(Cancellation {
            id: CancellationId(id),
            tier: TierId(tier),
            count: count(n)?,
            order,
            sale: sale.map(ManualSaleId),
            gone: count(gone)?,
            note,
            terms,
        });
    }
    Ok(out)
}
