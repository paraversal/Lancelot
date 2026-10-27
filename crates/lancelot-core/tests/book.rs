//! Rules of the in-memory event: defaults, names, edits, warnings and the lifecycle.

mod common;

use common::*;
use lancelot_core::*;

#[test]
fn new_event_has_default_accounts() {
    let book = new_book();
    let names = |kind| {
        book.accounts()
            .filter(|a| a.kind == kind)
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(AccountKind::Asset), ["Verein account", "Cash box"]);
    assert_eq!(
        names(AccountKind::Income),
        ["Tickets", "Drink donations", "Other"]
    );
    assert_eq!(
        names(AccountKind::Expense),
        ["Food", "Venue", "Drinks", "Pfand", "Tech", "Other"]
    );
    assert_eq!(book.event().state, EventState::Planning);
    assert_eq!(
        book.event().default_reimbursement_account,
        role(&book, Role::Verein)
    );
}

#[test]
fn event_needs_a_name() {
    let details = EventDetails {
        name: "  ".into(),
        starts_on: None,
        ends_on: None,
        location: None,
        notes: None,
    };
    assert_eq!(Book::new(details).unwrap_err().code(), "empty_name");
}

#[test]
fn names_are_unique_per_kind_ignoring_case() {
    let mut book = new_book();
    assert_eq!(
        book.add_account(AccountKind::Expense, " food ")
            .unwrap_err(),
        DomainError::DuplicateName {
            field: "name".into()
        }
    );
    // The same name is fine for another kind.
    assert!(book.add_account(AccountKind::Asset, "Food").is_ok());
    person(&mut book, "Anna");
    assert_eq!(
        book.add_person("ANNA", Person::default())
            .unwrap_err()
            .code(),
        "duplicate_name"
    );
    // Renaming to its own name, in another case, is allowed.
    let tech = expense(&book, "Tech");
    book.rename_account(tech, "TECH").unwrap();
    assert_eq!(book.account(tech).unwrap().name, "TECH");
}

#[test]
fn persons_and_opening_cannot_be_added_as_plain_accounts() {
    let mut book = new_book();
    assert_eq!(
        book.add_account(AccountKind::Person, "Anna")
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
    assert_eq!(
        book.add_account(AccountKind::Opening, "More")
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
}

#[test]
fn person_details_are_cleaned_and_checked() {
    let mut book = new_book();
    let iban = validate_iban("DE89 3704 0044 0532 0130 00").unwrap();
    let anna = book
        .add_person(
            "Anna",
            Person {
                holder: Some(" Anna Example ".into()),
                iban: Some(iban.clone()),
                bic: Some("cobadeffxxx".into()),
                contact: Some("".into()),
                notes: None,
            },
        )
        .unwrap();
    let p = book.person(anna).unwrap();
    assert_eq!(p.holder.as_deref(), Some("Anna Example"));
    assert_eq!(p.bic.as_deref(), Some("COBADEFFXXX"));
    assert_eq!(p.contact, None);
    assert_eq!(p.iban, Some(iban));

    let bad = Person {
        bic: Some("nope".into()),
        ..Person::default()
    };
    assert_eq!(
        book.update_person(anna, "Anna", bad).unwrap_err(),
        DomainError::InvalidBic
    );
}

#[test]
fn failed_changes_leave_the_book_untouched() {
    let mut book = new_book();
    let food = expense(&book, "Food");
    let before = book.clone();
    let err = book.save_txn(
        &draft(
            "Nothing",
            Entry::Expense {
                payer: food,
                lines: vec![split(food, "1")],
            },
        ),
        None,
    );
    assert_eq!(err.unwrap_err().code(), "wrong_account_kind");
    assert_eq!(book, before);
}

#[test]
fn editing_replaces_and_deleting_removes() {
    let mut book = new_book();
    let (cash, food, tech) = (
        role(&book, Role::CashBox),
        expense(&book, "Food"),
        expense(&book, "Tech"),
    );
    let id = save(
        &mut book,
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    book.save_txn(
        &draft(
            "Cables",
            Entry::Expense {
                payer: cash,
                lines: vec![split(tech, "35")],
            },
        ),
        Some(id),
    )
    .unwrap();
    assert_eq!(book.txns().count(), 1);
    assert_eq!(book.txn(id).unwrap().description, "Cables");
    assert_eq!(book.ledger().balance(food), Cents::ZERO);
    assert_eq!(book.ledger().balance(tech), eur("35"));

    book.delete_txn(id).unwrap();
    assert_eq!(book.ledger().balance(cash), Cents::ZERO);
    assert_eq!(book.delete_txn(id).unwrap_err(), DomainError::UnknownTxn);
    let missing = book.save_txn(
        &draft(
            "x",
            Entry::Transfer {
                from: cash,
                to: role(&book, Role::Verein),
                amount: eur("1"),
            },
        ),
        Some(TxnId(99)),
    );
    assert_eq!(missing.unwrap_err(), DomainError::UnknownTxn);
}

#[test]
fn overpaying_a_settle_is_allowed_with_a_warning() {
    let mut book = new_book();
    let (verein, food) = (role(&book, Role::Verein), expense(&book, "Food"));
    let malte = person(&mut book, "Malte");
    save(
        &mut book,
        "Cables",
        Entry::Expense {
            payer: malte,
            lines: vec![split(food, "110")],
        },
    );
    let saved = book
        .save_txn(
            &draft(
                "Reimbursement",
                Entry::Settle {
                    person: malte,
                    account: verein,
                    amount: eur("150"),
                    direction: SettleDirection::Reimburse,
                },
            ),
            None,
        )
        .unwrap();
    assert_eq!(
        saved.warnings,
        vec![Warning::SettleExceedsBalance {
            person: malte,
            net_after: eur("-40")
        }]
    );
}

#[test]
fn reimbursement_account_must_be_an_asset() {
    let mut book = new_book();
    let cash = role(&book, Role::CashBox);
    book.set_default_reimbursement_account(cash).unwrap();
    assert_eq!(book.event().default_reimbursement_account, cash);
    let food = expense(&book, "Food");
    assert_eq!(
        book.set_default_reimbursement_account(food)
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
}

#[test]
fn closing_needs_open_items_resolved_or_overridden() {
    let mut book = new_book();
    let food = expense(&book, "Food");
    let anna = person(&mut book, "Anna");
    save(
        &mut book,
        "Groceries",
        Entry::Expense {
            payer: anna,
            lines: vec![split(food, "12")],
        },
    );
    book.set_state(EventState::Settling, vec![]).unwrap();

    let item = OpenItem::PersonBalance {
        person: anna,
        net: eur("12"),
    };
    assert_eq!(
        book.set_state(EventState::Closed, vec![]).unwrap_err(),
        DomainError::ClosingBlocked {
            blockers: vec![item.clone()]
        }
    );
    let no_note = BlockerOverride {
        item: item.clone(),
        note: " ".into(),
    };
    assert_eq!(
        book.set_state(EventState::Closed, vec![no_note])
            .unwrap_err(),
        DomainError::MissingNote
    );

    let accepted = BlockerOverride {
        item,
        note: "Anna donates it".into(),
    };
    book.set_state(EventState::Closed, vec![accepted.clone()])
        .unwrap();
    assert_eq!(book.event().state, EventState::Closed);
    assert_eq!(book.event().closing_overrides, vec![accepted]);
}

#[test]
fn a_closed_event_is_read_only_until_reopened() {
    let mut book = new_book();
    book.set_state(EventState::Closed, vec![]).unwrap();
    let cash = role(&book, Role::CashBox);
    assert_eq!(
        book.save_txn(
            &draft(
                "x",
                Entry::Opening {
                    account: cash,
                    amount: eur("1")
                }
            ),
            None
        )
        .unwrap_err(),
        DomainError::EventClosed
    );
    assert_eq!(
        book.add_account(AccountKind::Expense, "Misc").unwrap_err(),
        DomainError::EventClosed
    );
    assert_eq!(
        book.set_state(EventState::Running, vec![]).unwrap_err(),
        DomainError::InvalidStateChange
    );

    book.set_state(EventState::Settling, vec![]).unwrap();
    assert!(book.event().closing_overrides.is_empty());
    assert!(
        book.save_txn(
            &draft(
                "x",
                Entry::Opening {
                    account: cash,
                    amount: eur("1")
                }
            ),
            None
        )
        .is_ok()
    );
}
