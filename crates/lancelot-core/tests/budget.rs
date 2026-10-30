//! Budget lines, links and the forecast (R-BUD-1 to R-BUD-5).

mod common;

use common::*;
use lancelot_core::*;
use proptest::prelude::*;
use tempfile::TempDir;

fn amount(category: AccountId, description: &str, estimate: &str) -> BudgetLineDraft {
    BudgetLineDraft {
        category,
        description: description.into(),
        estimate: Estimate::Amount {
            amount: eur(estimate),
        },
        complete: false,
    }
}

fn linked(account: AccountId, value: &str, line: BudgetLineId) -> SplitLine {
    SplitLine {
        budget_line: Some(line),
        ..split(account, value)
    }
}

fn spend(book: &mut Book, lines: Vec<SplitLine>) -> TxnId {
    let payer = role(book, Role::Verein);
    save(book, "Receipt", Entry::Expense { payer, lines })
}

fn earn(book: &mut Book, lines: Vec<SplitLine>) -> TxnId {
    let destination = role(book, Role::CashBox);
    save(book, "Takings", Entry::Income { destination, lines })
}

#[test]
fn an_expense_line_forecasts_the_larger_of_estimate_and_actual() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let food = book
        .add_budget_line(amount(kitchen, "Food", "1.000"))
        .unwrap();
    let figures = |b: &Book| *b.budget().line(food).unwrap();

    assert_eq!(figures(&book).forecast, eur("1.000"));
    spend(&mut book, vec![linked(kitchen, "400", food)]);
    assert_eq!(figures(&book).actual, eur("400"));
    assert_eq!(figures(&book).forecast, eur("1.000"));
    spend(&mut book, vec![linked(kitchen, "700", food)]);
    assert_eq!(figures(&book).forecast, eur("1.100"));

    // Once complete, only what was actually spent counts.
    book.update_budget_line(
        food,
        BudgetLineDraft {
            complete: true,
            ..amount(kitchen, "Food", "1.000")
        },
    )
    .unwrap();
    assert_eq!(figures(&book).forecast, eur("1.100"));
    book.update_budget_line(
        food,
        BudgetLineDraft {
            complete: true,
            ..amount(kitchen, "Food", "2.000")
        },
    )
    .unwrap();
    assert_eq!(figures(&book).forecast, eur("1.100"));
}

#[test]
fn an_income_line_forecasts_the_smaller_of_estimate_and_actual() {
    let mut book = new_book();
    let donations = income(&book, "Drink donations");
    let bar = book
        .add_budget_line(amount(donations, "Bar", "2.000"))
        .unwrap();
    let figures = |b: &Book| *b.budget().line(bar).unwrap();

    assert_eq!(figures(&book).forecast, Cents::ZERO);
    earn(&mut book, vec![linked(donations, "500", bar)]);
    assert_eq!(figures(&book).actual, eur("500"));
    assert_eq!(figures(&book).forecast, eur("500"));
    earn(&mut book, vec![linked(donations, "1.800", bar)]);
    assert_eq!(figures(&book).forecast, eur("2.000"));
}

#[test]
fn categories_add_unbudgeted_actuals_on_top() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let tickets = income(&book, "Tickets");
    let food = book
        .add_budget_line(amount(kitchen, "Food", "1.000"))
        .unwrap();
    book.add_budget_line(amount(kitchen, "Gas", "50")).unwrap();
    book.add_budget_line(amount(tickets, "Presale", "3.000"))
        .unwrap();
    spend(
        &mut book,
        vec![linked(kitchen, "300", food), split(kitchen, "20")],
    );
    earn(&mut book, vec![split(tickets, "2.500")]);

    let budget = book.budget();
    let k = budget.category(kitchen).unwrap();
    assert_eq!(k.figures.estimate, eur("1.050"));
    assert_eq!(k.figures.actual, eur("320"));
    assert_eq!(k.unbudgeted, eur("20"));
    assert_eq!(k.figures.forecast, eur("1.070"));
    // Ticket money linked to no line counts on top of the line's (empty) actual.
    let t = budget.category(tickets).unwrap();
    assert_eq!(t.unbudgeted, eur("2.500"));
    assert_eq!(t.figures.forecast, eur("2.500"));
    // Categories without budget lines forecast what they have.
    let venue = budget.category(expense(&book, "Venue")).unwrap();
    assert_eq!(venue.figures, Figures::default());

    assert_eq!(budget.expense.forecast, eur("1.070"));
    assert_eq!(budget.income.forecast, eur("2.500"));
    assert_eq!(budget.result.estimate, eur("1.950"));
    assert_eq!(budget.result.actual, book.ledger().result());
    assert_eq!(budget.result.forecast, eur("1.430"));
}

#[test]
fn an_estimate_can_be_quantity_times_unit_price() {
    let mut book = new_book();
    let drinks = expense(&book, "Drinks");
    let crates = book
        .add_budget_line(BudgetLineDraft {
            estimate: Estimate::Quantity {
                quantity: 40,
                unit_price: eur("12,49"),
            },
            ..amount(drinks, "Crates", "0")
        })
        .unwrap();
    assert_eq!(book.budget().line(crates).unwrap().estimate, eur("499,60"));
}

#[test]
fn budget_lines_never_change_a_balance() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    spend(&mut book, vec![split(kitchen, "120")]);
    let before = book.ledger().clone();
    let food = book
        .add_budget_line(amount(kitchen, "Food", "5.000"))
        .unwrap();
    assert_eq!(book.ledger(), &before);
    book.delete_budget_line(food).unwrap();
    assert_eq!(book.ledger(), &before);
}

#[test]
fn rejects_invalid_budget_lines() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let verein = role(&book, Role::Verein);
    let err = |book: &mut Book, d: BudgetLineDraft| book.add_budget_line(d).unwrap_err();

    assert_eq!(
        err(&mut book, amount(verein, "Food", "10")),
        DomainError::WrongAccountKind {
            field: "category".into()
        }
    );
    assert_eq!(
        err(&mut book, amount(kitchen, "  ", "10")),
        DomainError::EmptyName {
            field: "description".into()
        }
    );
    assert_eq!(
        err(&mut book, amount(kitchen, "Food", "-10")),
        DomainError::NegativeAmount {
            field: "estimate.amount".into()
        }
    );
    let quantity = |quantity, unit_price| BudgetLineDraft {
        estimate: Estimate::Quantity {
            quantity,
            unit_price,
        },
        ..amount(kitchen, "Food", "0")
    };
    assert_eq!(
        err(&mut book, quantity(0, eur("1"))),
        DomainError::NonPositiveAmount {
            field: "estimate.quantity".into()
        }
    );
    assert_eq!(
        err(&mut book, quantity(1, eur("-1"))).code(),
        "negative_amount"
    );
    assert_eq!(
        err(&mut book, quantity(u32::MAX, eur("10.000.000"))).code(),
        "overflow"
    );
    assert_eq!(book.budget_lines().count(), 0);
}

#[test]
fn a_link_stays_within_its_category() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let tech = expense(&book, "Tech");
    let food = book
        .add_budget_line(amount(kitchen, "Food", "100"))
        .unwrap();
    let payer = role(&book, Role::Verein);
    let receipt = |lines| draft("Receipt", Entry::Expense { payer, lines });

    let wrong = book.save_txn(
        &receipt(vec![split(kitchen, "5"), linked(tech, "10", food)]),
        None,
    );
    assert_eq!(
        wrong.unwrap_err(),
        DomainError::WrongBudgetLine {
            field: "lines[1].budgetLine".into()
        }
    );
    let unknown = book.save_txn(
        &receipt(vec![linked(kitchen, "10", BudgetLineId(99))]),
        None,
    );
    assert_eq!(unknown.unwrap_err(), DomainError::UnknownBudgetLine);

    // The category of a line in use cannot change, its estimate can.
    spend(&mut book, vec![linked(kitchen, "10", food)]);
    assert_eq!(
        book.update_budget_line(food, amount(tech, "Food", "100"))
            .unwrap_err(),
        DomainError::BudgetLineInUse
    );
    book.update_budget_line(food, amount(kitchen, "Food", "150"))
        .unwrap();
}

#[test]
fn deleting_a_budget_line_leaves_its_actuals_unbudgeted() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let food = book
        .add_budget_line(amount(kitchen, "Food", "100"))
        .unwrap();
    let txn = spend(&mut book, vec![linked(kitchen, "30", food)]);

    book.delete_budget_line(food).unwrap();
    assert!(
        book.txn(txn)
            .unwrap()
            .lines
            .iter()
            .all(|l| l.budget_line.is_none())
    );
    let k = book.budget().category(kitchen).unwrap();
    assert_eq!(k.unbudgeted, eur("30"));
    assert_eq!(k.figures.forecast, eur("30"));
    assert_eq!(
        book.delete_budget_line(food).unwrap_err(),
        DomainError::UnknownBudgetLine
    );
    // Ids are never reused.
    let next = book
        .add_budget_line(amount(kitchen, "Food", "100"))
        .unwrap();
    assert_eq!(next, BudgetLineId(food.0 + 1));
}

#[test]
fn a_closed_event_keeps_its_budget() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let food = book
        .add_budget_line(amount(kitchen, "Food", "100"))
        .unwrap();
    let cash = role(&book, Role::CashBox);
    book.set_closing_count(cash, june(14), Cents::ZERO).unwrap();
    book.close(Vec::new(), now()).unwrap();
    assert_eq!(
        book.add_budget_line(amount(kitchen, "Gas", "10"))
            .unwrap_err(),
        DomainError::EventClosed
    );
    assert_eq!(
        book.delete_budget_line(food).unwrap_err(),
        DomainError::EventClosed
    );
}

#[test]
fn budget_lines_and_links_survive_saving_and_reopening() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("budget.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();

    let kitchen = expense(store.book(), "Kitchen");
    let drinks = expense(store.book(), "Drinks");
    let food = store
        .add_budget_line(amount(kitchen, "Food", "1.000"))
        .unwrap();
    let crates = store
        .add_budget_line(BudgetLineDraft {
            estimate: Estimate::Quantity {
                quantity: 40,
                unit_price: eur("12,50"),
            },
            complete: true,
            ..amount(drinks, "Crates", "0")
        })
        .unwrap();
    let payer = role(store.book(), Role::Verein);
    store
        .save_txn(
            &draft(
                "Groceries",
                Entry::Expense {
                    payer,
                    lines: vec![
                        linked(kitchen, "312,40", food),
                        linked(drinks, "480", crates),
                    ],
                },
            ),
            None,
        )
        .unwrap();
    let gone = store.add_budget_line(amount(kitchen, "Gas", "50")).unwrap();
    store.delete_budget_line(gone).unwrap();

    let book = store.book().clone();
    drop(store);
    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book(), &book);
    assert_eq!(store.book().budget().expense.forecast, eur("1.480"));

    let log = store.change_log(50).unwrap();
    let ops: Vec<_> = log
        .iter()
        .filter(|e| e.entity == "budget_line")
        .map(|e| e.op.as_str())
        .collect();
    assert_eq!(ops.len(), 4, "{ops:?}");
    assert!(ops.contains(&"delete"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// R-BUD-3 to R-BUD-5 for random budgets and linked entries.
    #[test]
    fn the_forecast_follows_its_rules(
        plans in prop::collection::vec((0usize..10, 0i64..500_000, any::<bool>()), 0..8),
        entries in prop::collection::vec((0usize..10, 1i64..300_000, 0usize..9), 0..20),
    ) {
        let mut book = new_book();
        let categories: Vec<AccountId> = book
            .accounts()
            .filter(|a| matches!(a.kind, AccountKind::Income | AccountKind::Expense))
            .map(|a| a.id)
            .collect();
        let mut lines = Vec::new();
        for (i, (c, cents, complete)) in plans.iter().enumerate() {
            let category = categories[c % categories.len()];
            let id = book
                .add_budget_line(BudgetLineDraft {
                    category,
                    description: format!("line {i}"),
                    estimate: Estimate::Amount { amount: Cents::new(*cents).unwrap() },
                    complete: *complete,
                })
                .unwrap();
            lines.push((id, category));
        }
        for (c, cents, link) in &entries {
            let category = categories[c % categories.len()];
            // Link to a line of this category when one is picked, else leave it unbudgeted.
            let budget_line = lines
                .get(*link)
                .filter(|(_, cat)| *cat == category)
                .map(|(id, _)| *id);
            let line = SplitLine {
                account: category,
                amount: Cents::new(*cents).unwrap(),
                memo: None,
                budget_line,
            };
            if book.account(category).unwrap().kind == AccountKind::Income {
                earn(&mut book, vec![line]);
            } else {
                spend(&mut book, vec![line]);
            }
        }

        let ledger = book.ledger().clone();
        let budget = book.budget().clone();
        prop_assert_eq!(budget.income.actual, ledger.income_total());
        prop_assert_eq!(budget.expense.actual, ledger.expense_total());
        prop_assert_eq!(budget.result.actual, ledger.result());

        for c in &budget.categories {
            let own: Vec<_> = book
                .budget_lines()
                .filter(|b| b.draft.category == c.category)
                .map(|b| *budget.line(b.id).unwrap())
                .collect();
            let sum = |f: fn(&Figures) -> Cents| own.iter().map(f).fold(Cents::ZERO, |a, b| a.checked_add(b).unwrap());
            prop_assert_eq!(c.figures.estimate, sum(|f| f.estimate));
            prop_assert_eq!(c.figures.actual, sum(|f| f.actual).checked_add(c.unbudgeted).unwrap());
            prop_assert_eq!(c.figures.forecast, sum(|f| f.forecast).checked_add(c.unbudgeted).unwrap());
        }
        for b in book.budget_lines() {
            let f = budget.line(b.id).unwrap();
            let account = book.account(b.draft.category).unwrap();
            let is_income = account.kind == AccountKind::Income;
            // The ticket categories forecast what is booked, whatever the estimate.
            let follows_the_tickets = matches!(
                account.role,
                Some(Role::Tickets | Role::TicketFees | Role::TicketRefunds)
            );
            let expected = match (b.draft.complete || follows_the_tickets, is_income) {
                (true, _) => f.actual,
                (false, true) => f.estimate.min(f.actual),
                (false, false) => f.estimate.max(f.actual),
            };
            prop_assert_eq!(f.forecast, expected);
        }

        // Without any budget lines, the ledger is untouched and the forecast is the actual.
        let ids: Vec<_> = book.budget_lines().map(|b| b.id).collect();
        for id in ids {
            book.delete_budget_line(id).unwrap();
        }
        prop_assert_eq!(book.ledger(), &ledger);
        prop_assert_eq!(book.budget().result.forecast, ledger.result());
    }
}
