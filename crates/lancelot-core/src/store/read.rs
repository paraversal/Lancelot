//! Reads an event file into [`BookParts`].

use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension};

use super::StoreError;
use super::codec::{text_date, text_enum, text_timestamp};
use crate::book::{BookParts, NextIds};
use crate::iban::validate_iban;
use crate::model::{
    Account, AccountId, Attachment, AttachmentId, ClosingCount, Event, EventDetails, Line, Person,
    Txn, TxnId,
};
use crate::money::Cents;

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
    let mut stmt = conn.prepare("SELECT id, kind, name, role FROM account ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (id, kind, name, role) = row?;
        let id = AccountId(id);
        let role = role.as_deref().map(text_enum).transpose()?;
        out.insert(
            id,
            Account {
                id,
                kind: text_enum(&kind)?,
                name,
                role,
            },
        );
    }
    Ok(out)
}

/// All transactions with their lines, ordered by id and line position.
pub(crate) fn txns(conn: &Connection) -> Result<Vec<Txn>, StoreError> {
    let mut stmt = conn
        .prepare("SELECT id, kind, date, description, counterparty, notes FROM txn ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut txns = BTreeMap::new();
    for row in rows {
        let (id, kind, date, description, counterparty, notes) = row?;
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
            },
        );
    }

    let mut stmt = conn
        .prepare("SELECT txn_id, account_id, amount, memo FROM line ORDER BY txn_id, position")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in rows {
        let (txn, account, amount, memo) = row?;
        let txn = txns
            .get_mut(&TxnId(txn))
            .ok_or_else(|| StoreError::corrupt(format!("line of unknown transaction {txn}")))?;
        txn.lines.push(Line {
            account: AccountId(account),
            amount: cents(amount)?,
            memo,
        });
    }
    Ok(txns.into_values().collect())
}

pub(crate) fn parts(conn: &Connection) -> Result<BookParts, StoreError> {
    let event = conn.query_row(
        "SELECT name, starts_on, ends_on, location, notes, state, default_reimbursement_account_id, closing_overrides
         FROM event WHERE id = 1",
        [],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
            ))
        },
    )?;
    let (name, starts_on, ends_on, location, notes, state, default_account, overrides) = event;
    let event = Event {
        details: EventDetails {
            name,
            starts_on: starts_on.as_deref().map(text_date).transpose()?,
            ends_on: ends_on.as_deref().map(text_date).transpose()?,
            location,
            notes,
        },
        state: text_enum(&state)?,
        default_reimbursement_account: AccountId(default_account),
        closing_overrides: serde_json::from_str(&overrides)?,
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

    Ok(BookParts {
        event,
        accounts: accounts(conn)?.into_values().collect(),
        persons,
        txns: txns(conn)?,
        attachments,
        closing_counts,
        next: NextIds {
            account: next_id(conn, "next_account_id")?,
            txn: next_id(conn, "next_txn_id")?,
            attachment: next_id(conn, "next_attachment_id")?,
        },
    })
}
