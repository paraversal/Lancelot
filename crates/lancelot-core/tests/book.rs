//! Rules of the in-memory event: defaults, names, edits, warnings and the lifecycle.

mod common;

use common::*;
use lancelot_core::*;
use time::OffsetDateTime;

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
    assert_eq!(names(AccountKind::Income), ["Tickets", "General"]);
    assert_eq!(
        names(AccountKind::Expense),
        ["Kitchen", "Mobility", "Venue", "Tech", "General"]
    );
    assert!(!book.event().is_closed());
    assert_eq!(
        book.event().default_reimbursement_account,
        asset(&book, "Verein account")
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
    assert_eq!(
        Book::new(details, &two_accounts()).unwrap_err().code(),
        "empty_name"
    );
}

fn account(name: &str, asset_type: AssetType) -> AccountDraft {
    AccountDraft {
        name: name.into(),
        asset_type,
    }
}

fn start_with(accounts: &[AccountDraft]) -> Result<Book, DomainError> {
    Book::new(new_book().event().details.clone(), accounts)
}

#[test]
fn event_needs_an_account_with_a_free_name() {
    let code = |accounts: &[AccountDraft]| start_with(accounts).unwrap_err().code();
    assert_eq!(code(&[]), "no_accounts");
    assert_eq!(code(&[account(" ", AssetType::Bank)]), "empty_name");
    assert_eq!(
        code(&[
            account("Giro", AssetType::Bank),
            account("GIRO ", AssetType::Cash)
        ]),
        "duplicate_name"
    );
}

#[test]
fn money_starts_out_at_the_first_bank_account() {
    let book = start_with(&[
        account("Till", AssetType::Cash),
        account("Giro", AssetType::Bank),
        account("Savings", AssetType::Bank),
    ])
    .unwrap();
    let giro = asset(&book, "Giro");
    assert_eq!(book.event().default_reimbursement_account, giro);
    assert_eq!(book.ticket_settings().account, Some(giro));
    assert!(book.account(giro).unwrap().net_movement);
    let till = book.account(asset(&book, "Till")).unwrap();
    assert_eq!(till.asset_type, Some(AssetType::Cash));
    assert!(!till.net_movement);
}

#[test]
fn a_cash_account_alone_is_enough() {
    let book = start_with(&[account("Till", AssetType::Cash)]).unwrap();
    let till = asset(&book, "Till");
    assert_eq!(book.event().default_reimbursement_account, till);
    assert_eq!(book.ticket_settings().account, Some(till));
}

#[test]
fn names_are_unique_per_kind_ignoring_case() {
    let mut book = new_book();
    assert_eq!(
        book.add_account(AccountKind::Expense, " kitchen ")
            .unwrap_err(),
        DomainError::DuplicateName {
            field: "name".into()
        }
    );
    // The same name is fine for another kind.
    assert!(book.add_account(AccountKind::Asset, "Kitchen").is_ok());
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
    let food = expense(&book, "Kitchen");
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
        asset(&book, "Cash box"),
        expense(&book, "Kitchen"),
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
                to: asset(&book, "Verein account"),
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
    let (verein, food) = (asset(&book, "Verein account"), expense(&book, "Kitchen"));
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
    let cash = asset(&book, "Cash box");
    book.set_default_reimbursement_account(cash).unwrap();
    assert_eq!(book.event().default_reimbursement_account, cash);
    let food = expense(&book, "Kitchen");
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
    let food = expense(&book, "Kitchen");
    let anna = person(&mut book, "Anna");
    let cash = asset(&book, "Cash box");
    let groceries = save(
        &mut book,
        "Groceries",
        Entry::Expense {
            payer: anna,
            lines: vec![split(food, "12")],
        },
    );
    let item = OpenItem::PersonBalance {
        person: anna,
        net: eur("12"),
    };
    assert_eq!(
        book.close(vec![], now()).unwrap_err(),
        DomainError::ClosingBlocked {
            blockers: vec![
                item.clone(),
                OpenItem::MissingReceipt { txn: groceries },
                OpenItem::ClosingCountMissing { account: cash },
            ]
        }
    );
    attach(&mut book, groceries);
    book.set_closing_count(cash, june(14), Cents::ZERO).unwrap();
    assert_eq!(
        book.close(vec![], now()).unwrap_err(),
        DomainError::ClosingBlocked {
            blockers: vec![item.clone()]
        }
    );
    let no_note = BlockerOverride {
        item: item.clone(),
        note: " ".into(),
    };
    assert_eq!(
        book.close(vec![no_note], now()).unwrap_err(),
        DomainError::MissingNote
    );

    let accepted = BlockerOverride {
        item,
        note: "Anna donates it".into(),
    };
    book.close(vec![accepted.clone()], now()).unwrap();
    assert!(book.event().is_closed());
    assert_eq!(book.event().closing_overrides, vec![accepted]);
}

#[test]
fn a_closed_event_is_read_only_until_reopened() {
    let mut book = new_book();
    let cash = asset(&book, "Cash box");
    book.set_closing_count(cash, june(14), Cents::ZERO).unwrap();
    book.close(vec![], now()).unwrap();
    // Closing twice does nothing.
    book.close(vec![], now()).unwrap();
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

    book.reopen().unwrap();
    assert!(!book.event().is_closed());
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

#[test]
fn closing_counts_show_the_difference() {
    let mut book = new_book();
    let (cash, verein, donations) = (
        asset(&book, "Cash box"),
        asset(&book, "Verein account"),
        income(&book, "General"),
    );
    save(
        &mut book,
        "Donations",
        Entry::Income {
            destination: cash,
            lines: vec![split(donations, "412,90")],
        },
    );

    let status = book.ledger().cash()[0];
    assert_eq!(
        (status.account, status.computed, status.counted),
        (cash, eur("412,90"), None)
    );
    assert_eq!(
        book.open_items(),
        vec![OpenItem::ClosingCountMissing { account: cash }]
    );

    book.set_closing_count(cash, june(14), eur("410")).unwrap();
    let status = book.ledger().cash()[0];
    assert_eq!(status.difference, Some(eur("-2,90")));
    assert_eq!(
        book.open_items(),
        vec![OpenItem::CashDifference {
            account: cash,
            difference: eur("-2,90")
        }]
    );

    book.set_closing_count(cash, june(14), eur("412,90"))
        .unwrap();
    assert!(book.open_items().is_empty());

    // Other asset accounts appear once they have a count; they never need one.
    assert_eq!(book.ledger().cash().len(), 1);
    book.set_closing_count(verein, june(14), Cents::ZERO)
        .unwrap();
    assert_eq!(book.ledger().cash().len(), 2);
    book.clear_closing_count(verein).unwrap();
    assert_eq!(book.ledger().cash().len(), 1);

    assert_eq!(
        book.set_closing_count(cash, june(14), eur("-1"))
            .unwrap_err()
            .code(),
        "negative_cash_count"
    );
    // A bank account can end below zero: it holds only the event's net movement (R-ISO-2).
    book.set_closing_count(verein, june(14), eur("-100"))
        .unwrap();
    let counted = book.ledger().cash().iter().find(|c| c.account == verein);
    assert_eq!(
        counted.unwrap().difference,
        Some(eur("-100")),
        "nothing has moved on the Verein account yet"
    );
    assert_eq!(
        book.set_asset_type(verein, AssetType::Cash)
            .unwrap_err()
            .code(),
        "negative_cash_count"
    );
    book.clear_closing_count(verein).unwrap();
    let food = expense(&book, "Kitchen");
    assert_eq!(
        book.set_closing_count(food, june(14), eur("1"))
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
}

#[test]
fn attachments_belong_to_transactions() {
    let mut book = new_book();
    let (cash, food) = (asset(&book, "Cash box"), expense(&book, "Kitchen"));
    let id = save(
        &mut book,
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "20")],
        },
    );
    assert_eq!(book.ledger().missing_receipts(), [id]);

    let a = attach(&mut book, id);
    assert!(book.ledger().missing_receipts().is_empty());
    assert_eq!(book.attachments_of(id).count(), 1);

    let too_big = book.add_attachment(
        id,
        "huge.pdf",
        "application/pdf",
        MAX_ATTACHMENT_SIZE + 1,
        "00",
        now(),
    );
    assert_eq!(too_big.unwrap_err(), DomainError::AttachmentTooLarge);
    assert_eq!(
        book.add_attachment(TxnId(99), "x.png", "image/png", 1, "00", now())
            .unwrap_err(),
        DomainError::UnknownTxn
    );

    book.remove_attachment(a).unwrap();
    assert_eq!(book.ledger().missing_receipts(), [id]);
    assert_eq!(
        book.remove_attachment(a).unwrap_err(),
        DomainError::UnknownAttachment
    );

    // Deleting a transaction takes its attachments with it.
    attach(&mut book, id);
    book.delete_txn(id).unwrap();
    assert_eq!(book.attachments().count(), 0);
}

#[test]
fn a_split_must_add_up_to_the_receipt_total() {
    let mut book = new_book();
    let (cash, kitchen, drinks) = (
        asset(&book, "Cash box"),
        expense(&book, "Kitchen"),
        expense(&book, "General"),
    );
    let mut d = draft(
        "Metro",
        Entry::Expense {
            payer: cash,
            lines: vec![split(kitchen, "248,90"), split(drinks, "63,50")],
        },
    );
    d.total = Some(eur("312,40"));
    assert!(book.save_txn(&d, None).is_ok());
    d.total = Some(eur("320"));
    let err = book.save_txn(&d, None).unwrap_err();
    assert_eq!((err.code(), err.field()), ("split_mismatch", Some("lines")));
    assert_eq!(book.txns().count(), 1);
    assert_eq!(
        txn_amount(&book.txns().next().unwrap().lines),
        eur("312,40")
    );
}

#[test]
fn asset_accounts_are_bank_or_cash() {
    let mut book = new_book();
    let types = |book: &Book| {
        book.accounts()
            .map(|a| (a.name.clone(), a.asset_type))
            .filter(|(_, t)| t.is_some())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        types(&book),
        [
            ("Verein account".into(), Some(AssetType::Bank)),
            ("Cash box".into(), Some(AssetType::Cash)),
        ]
    );
    assert!(
        book.accounts()
            .all(|a| a.asset_type.is_some() == (a.kind == AccountKind::Asset))
    );

    let bar = book.add_asset("Cash – Bar", AssetType::Cash).unwrap();
    let savings = book.add_account(AccountKind::Asset, "Savings").unwrap();
    assert_eq!(
        book.account(savings).unwrap().asset_type,
        Some(AssetType::Bank)
    );
    book.set_asset_type(savings, AssetType::Cash).unwrap();
    assert_eq!(
        book.account(savings).unwrap().asset_type,
        Some(AssetType::Cash)
    );
    assert_eq!(
        book.set_asset_type(expense(&book, "Kitchen"), AssetType::Cash)
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
    assert_eq!(
        book.add_asset("cash box", AssetType::Cash)
            .unwrap_err()
            .code(),
        "duplicate_name"
    );

    // Every cash account needs a closing count; bank accounts don't.
    let cash = asset(&book, "Cash box");
    let missing: Vec<_> = book
        .open_items()
        .into_iter()
        .filter(|i| matches!(i, OpenItem::ClosingCountMissing { .. }))
        .collect();
    assert_eq!(
        missing,
        [cash, bar, savings].map(|account| OpenItem::ClosingCountMissing { account })
    );
}

#[test]
fn an_opening_count_is_one_opening_transaction() {
    let mut book = new_book();
    let cash = asset(&book, "Cash box");
    let status = |book: &Book| {
        *book
            .ledger()
            .cash()
            .iter()
            .find(|c| c.account == cash)
            .unwrap()
    };
    assert_eq!(status(&book).opening, None);

    let txn = book.set_opening_count(cash, june(12), eur("80")).unwrap();
    let opening = status(&book).opening.unwrap();
    assert_eq!(
        (opening.txn, opening.date, opening.amount),
        (txn, june(12), eur("80"))
    );
    assert_eq!(book.txn(txn).unwrap().kind, TxnKind::Opening);
    assert_eq!(status(&book).computed, eur("80"));

    // A second opening entered by hand is merged into the count when it is set again.
    save(
        &mut book,
        "More change",
        Entry::Opening {
            account: cash,
            amount: eur("20"),
        },
    );
    assert_eq!(status(&book).opening.unwrap().amount, eur("100"));
    assert_eq!(
        book.set_opening_count(cash, june(11), eur("90")).unwrap(),
        txn
    );
    let opening = status(&book).opening.unwrap();
    assert_eq!((opening.date, opening.amount), (june(11), eur("90")));
    assert_eq!(
        book.txns().filter(|t| t.kind == TxnKind::Opening).count(),
        1
    );
    assert_eq!(
        book.set_opening_count(cash, june(11), Cents::ZERO)
            .unwrap_err()
            .code(),
        "non_positive_amount"
    );

    book.clear_opening_count(cash).unwrap();
    assert_eq!(status(&book).opening, None);
    assert_eq!(book.txns().count(), 0);

    // A bank account with an opening count shows up among the counts, but needs no closing count.
    let verein = asset(&book, "Verein account");
    book.set_opening_count(verein, june(12), eur("1000"))
        .unwrap();
    let bank = book
        .ledger()
        .cash()
        .iter()
        .find(|c| c.account == verein)
        .copied()
        .unwrap();
    assert_eq!(bank.asset_type, AssetType::Bank);
    assert!(
        !book
            .ledger()
            .open_items()
            .contains(&OpenItem::ClosingCountMissing { account: verein })
    );
}

#[test]
fn marking_payouts_paid_settles_only_those_with_an_iban() {
    let mut book = new_book();
    let kitchen = book
        .account_by_name(AccountKind::Expense, "Kitchen")
        .unwrap()
        .id;
    let verein = book
        .account_by_name(AccountKind::Asset, "Verein account")
        .unwrap()
        .id;
    let with_iban = Person {
        iban: Some(validate_iban("DE89370400440532013000").unwrap()),
        ..Person::default()
    };
    let anna = book.add_person("Anna", with_iban).unwrap();
    let bob = book.add_person("Bob", Person::default()).unwrap();
    for payer in [anna, bob] {
        let draft = TxnDraft {
            date: june(13),
            description: "Metro".into(),
            counterparty: None,
            notes: None,
            entry: Entry::Expense {
                payer,
                lines: vec![SplitLine {
                    account: kitchen,
                    amount: eur("20,00"),
                    budget_line: None,
                    cost_centre: None,
                }],
            },
            total: None,
        };
        book.save_txn(&draft, None).unwrap();
    }
    assert_eq!(book.mark_payouts_paid(verein, june(20)), Ok(1));
    assert!(book.ledger().person(anna).unwrap().net.is_zero());
    assert_eq!(book.ledger().person(bob).unwrap().net, eur("20,00"));
}

#[test]
fn setting_both_counts_is_one_change_that_fails_whole() {
    let mut book = new_book();
    let cash = book
        .account_by_name(AccountKind::Asset, "Cash box")
        .unwrap()
        .id;
    book.set_counts(
        cash,
        Some((june(12), eur("50,00"))),
        Some((june(14), eur("45,00"))),
    )
    .unwrap();
    assert_eq!(book.closing_count(cash).unwrap().amount, eur("45,00"));
    let before = book.clone();

    let negative = Cents::ZERO.checked_sub(eur("1,00")).unwrap();
    assert!(
        book.set_counts(
            cash,
            Some((june(12), eur("60,00"))),
            Some((june(14), negative))
        )
        .is_err()
    );
    assert_eq!(book, before);
    assert!(
        book.set_counts(cash, Some((june(12), negative)), None)
            .is_err()
    );
    assert_eq!(book, before);

    book.set_counts(cash, None, None).unwrap();
    assert!(book.closing_count(cash).is_none());
    assert!(book.txns().next().is_none());

    let kitchen = book
        .account_by_name(AccountKind::Expense, "Kitchen")
        .unwrap()
        .id;
    assert!(book.set_counts(kitchen, None, None).is_err());
}

#[test]
fn reopening_keeps_the_notes_for_closing_again() {
    let mut book = new_book();
    let anna = book.add_person("Anna", Person::default()).unwrap();
    let cash = asset(&book, "Cash box");
    let food = expense(&book, "Kitchen");
    save(
        &mut book,
        "Groceries",
        Entry::Expense {
            payer: anna,
            lines: vec![split(food, "20")],
        },
    );
    let overrides: Vec<_> = book
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "fine".into(),
        })
        .collect();
    book.close(overrides.clone(), OffsetDateTime::UNIX_EPOCH)
        .unwrap();
    book.reopen().unwrap();
    assert_eq!(book.event().closing_overrides, overrides);
    assert!(book.close(Vec::new(), OffsetDateTime::UNIX_EPOCH).is_err());

    // What is accepted is what the second closing says.
    book.set_closing_count(cash, june(30), Cents::ZERO).unwrap();
    let kept: Vec<_> = book
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "still fine".into(),
        })
        .collect();
    book.close(kept.clone(), OffsetDateTime::UNIX_EPOCH)
        .unwrap();
    assert_eq!(book.event().closing_overrides, kept);
}
