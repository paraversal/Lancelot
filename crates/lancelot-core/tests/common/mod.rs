//! Helpers shared by the integration tests.
#![allow(dead_code)]

pub mod mock;
pub mod world;

use lancelot_core::*;
use time::{Date, Month, OffsetDateTime};

/// An amount written as the treasurer would type it, e.g. `eur("312,40")`.
pub fn eur(text: &str) -> Cents {
    parse_amount(text, NumberFormat::De, true).unwrap().cents
}

/// A day in June 2026, the month of the example event.
pub fn june(day: u8) -> Date {
    Date::from_calendar_date(2026, Month::June, day).unwrap()
}

pub fn new_book() -> Book {
    Book::new(EventDetails {
        name: "Sommerfest 2026".into(),
        starts_on: Some(june(12)),
        ends_on: Some(june(14)),
        location: None,
        notes: None,
    })
    .unwrap()
}

pub fn draft(description: &str, entry: Entry) -> TxnDraft {
    TxnDraft {
        date: june(13),
        description: description.into(),
        counterparty: None,
        notes: None,
        entry,
        total: None,
    }
}

pub fn split(account: AccountId, amount: &str) -> SplitLine {
    SplitLine {
        account,
        amount: eur(amount),
        memo: None,
        budget_line: None,
    }
}

pub fn role(book: &Book, role: Role) -> AccountId {
    book.account_by_role(role).unwrap().id
}

pub fn category(book: &Book, kind: AccountKind, name: &str) -> AccountId {
    book.account_by_name(kind, name).unwrap().id
}

pub fn expense(book: &Book, name: &str) -> AccountId {
    category(book, AccountKind::Expense, name)
}

pub fn income(book: &Book, name: &str) -> AccountId {
    category(book, AccountKind::Income, name)
}

pub fn person(book: &mut Book, name: &str) -> AccountId {
    book.add_person(name, Person::default()).unwrap()
}

/// Saves a transaction and returns its id, failing the test on any error.
pub fn save(book: &mut Book, description: &str, entry: Entry) -> TxnId {
    book.save_txn(&draft(description, entry), None).unwrap().id
}

/// The sum of all raw balances. Always zero.
pub fn raw_sum(book: &Book) -> i64 {
    book.accounts()
        .map(|a| book.ledger().raw_balance(a.id).get())
        .sum()
}

/// A fixed point in time during the example event.
pub fn now() -> OffsetDateTime {
    june(14).with_hms(18, 0, 0).unwrap().assume_utc()
}

/// Records a made-up receipt on a transaction.
pub fn attach(book: &mut Book, txn: TxnId) -> AttachmentId {
    book.add_attachment(txn, "receipt.jpg", "image/jpeg", 1024, "00", now())
        .unwrap()
}
