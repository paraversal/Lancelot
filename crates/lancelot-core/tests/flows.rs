//! The money-flow table in requirements §4.1, one test per row, plus the netting example.

mod common;

use common::*;
use lancelot_core::*;

fn bal(book: &Book, account: AccountId) -> Cents {
    book.ledger().balance(account)
}

#[test]
fn ticket_revenue_arrives_at_the_verein_account() {
    let mut book = new_book();
    let (verein, tickets) = (role(&book, Role::Verein), income(&book, "Tickets"));
    save(
        &mut book,
        "Tickets",
        Entry::Income {
            destination: verein,
            lines: vec![split(tickets, "6.227,50")],
        },
    );
    assert_eq!(bal(&book, verein), eur("6.227,50"));
    assert_eq!(bal(&book, tickets), eur("6.227,50"));
    assert_eq!(book.ledger().result(), eur("6.227,50"));
}

#[test]
fn ticket_sold_outside_regular_channels_is_held_by_the_person() {
    let mut book = new_book();
    let tickets = income(&book, "Tickets");
    let bob = person(&mut book, "Bob");
    save(
        &mut book,
        "Tickets",
        Entry::Income {
            destination: bob,
            lines: vec![split(tickets, "300")],
        },
    );
    let p = book.ledger().person(bob).unwrap();
    assert_eq!(
        (p.held, p.net, p.status),
        (eur("300"), eur("-300"), PersonStatus::Owes)
    );
}

#[test]
fn person_forwards_held_money_to_the_verein_account() {
    let mut book = new_book();
    let (verein, tickets) = (role(&book, Role::Verein), income(&book, "Tickets"));
    let bob = person(&mut book, "Bob");
    save(
        &mut book,
        "Tickets",
        Entry::Income {
            destination: bob,
            lines: vec![split(tickets, "300")],
        },
    );
    save(
        &mut book,
        "Forwarded",
        Entry::Settle {
            person: bob,
            account: verein,
            amount: eur("300"),
            direction: SettleDirection::Repay,
        },
    );
    let p = book.ledger().person(bob).unwrap();
    assert_eq!(
        (p.repaid, p.net, p.status),
        (eur("300"), Cents::ZERO, PersonStatus::Settled)
    );
    assert_eq!(bal(&book, verein), eur("300"));
}

#[test]
fn cash_box_opening_count() {
    let mut book = new_book();
    let cash = role(&book, Role::CashBox);
    save(
        &mut book,
        "Opening count",
        Entry::Opening {
            account: cash,
            amount: eur("80"),
        },
    );
    assert_eq!(bal(&book, cash), eur("80"));
    assert_eq!(bal(&book, role(&book, Role::Opening)), eur("80"));
    // An opening count is not income.
    assert_eq!(book.ledger().result(), Cents::ZERO);
    assert_eq!(book.ledger().where_the_money_is().total, Cents::ZERO);
}

#[test]
fn person_puts_own_money_into_the_cash_box() {
    let mut book = new_book();
    let cash = role(&book, Role::CashBox);
    let anna = person(&mut book, "Anna");
    save(
        &mut book,
        "Change money",
        Entry::Transfer {
            from: anna,
            to: cash,
            amount: eur("200"),
        },
    );
    let p = book.ledger().person(anna).unwrap();
    assert_eq!(
        (p.advanced, p.net, p.status),
        (eur("200"), eur("200"), PersonStatus::Owed)
    );
    assert_eq!(bal(&book, cash), eur("200"));
}

#[test]
fn person_pays_for_food_with_own_card() {
    let mut book = new_book();
    let food = expense(&book, "Food");
    let anna = person(&mut book, "Anna");
    save(
        &mut book,
        "Groceries",
        Entry::Expense {
            payer: anna,
            lines: vec![split(food, "312,40")],
        },
    );
    assert_eq!(book.ledger().person(anna).unwrap().net, eur("312,40"));
    assert_eq!(bal(&book, food), eur("312,40"));
    assert_eq!(book.ledger().result(), eur("-312,40"));
}

#[test]
fn food_bought_from_the_cash_box() {
    let mut book = new_book();
    let (cash, food) = (role(&book, Role::CashBox), expense(&book, "Food"));
    save(
        &mut book,
        "Bakery",
        Entry::Expense {
            payer: cash,
            lines: vec![split(food, "26,35")],
        },
    );
    assert_eq!(bal(&book, cash), eur("-26,35"));
}

#[test]
fn venue_invoice_paid_by_the_verein() {
    let mut book = new_book();
    let (verein, venue) = (role(&book, Role::Verein), expense(&book, "Venue"));
    save(
        &mut book,
        "Hall",
        Entry::Expense {
            payer: verein,
            lines: vec![split(venue, "1.800")],
        },
    );
    assert_eq!(bal(&book, verein), eur("-1.800"));
    assert_eq!(book.ledger().expense_total(), eur("1.800"));
}

#[test]
fn person_reimbursed() {
    let mut book = new_book();
    let (verein, food) = (role(&book, Role::Verein), expense(&book, "Food"));
    let anna = person(&mut book, "Anna");
    save(
        &mut book,
        "Groceries",
        Entry::Expense {
            payer: anna,
            lines: vec![split(food, "312,40")],
        },
    );
    let saved = book
        .save_txn(
            &draft(
                "Reimbursement",
                Entry::Settle {
                    person: anna,
                    account: verein,
                    amount: eur("312,40"),
                    direction: SettleDirection::Reimburse,
                },
            ),
            None,
        )
        .unwrap();
    assert!(saved.warnings.is_empty());
    let p = book.ledger().person(anna).unwrap();
    assert_eq!(
        (p.reimbursed, p.status),
        (eur("312,40"), PersonStatus::Settled)
    );
    assert_eq!(bal(&book, verein), eur("-312,40"));
}

#[test]
fn drink_donations_collected_in_cash() {
    let mut book = new_book();
    let (cash, donations) = (role(&book, Role::CashBox), income(&book, "Drink donations"));
    save(
        &mut book,
        "Donations",
        Entry::Income {
            destination: cash,
            lines: vec![split(donations, "812,50")],
        },
    );
    assert_eq!(bal(&book, cash), eur("812,50"));
    assert_eq!(book.ledger().income_total(), eur("812,50"));
}

#[test]
fn netting_example_bob() {
    // Requirements §4.1: Bob received €300 of ticket money and advanced €120 for food.
    let mut book = new_book();
    let (verein, tickets, food) = (
        role(&book, Role::Verein),
        income(&book, "Tickets"),
        expense(&book, "Food"),
    );
    let bob = person(&mut book, "Bob");
    save(
        &mut book,
        "Tickets",
        Entry::Income {
            destination: bob,
            lines: vec![split(tickets, "300")],
        },
    );
    save(
        &mut book,
        "Food",
        Entry::Expense {
            payer: bob,
            lines: vec![split(food, "120")],
        },
    );

    let p = *book.ledger().person(bob).unwrap();
    assert_eq!((p.advanced, p.held), (eur("120"), eur("300")));
    assert_eq!((p.net, p.status), (eur("-180"), PersonStatus::Owes));
    assert_eq!(book.ledger().balance(bob), eur("-180"));

    save(
        &mut book,
        "Bob pays back",
        Entry::Settle {
            person: bob,
            account: verein,
            amount: eur("180"),
            direction: SettleDirection::Repay,
        },
    );
    assert_eq!(
        book.ledger().person(bob).unwrap().status,
        PersonStatus::Settled
    );
    assert_eq!(raw_sum(&book), 0);
}
