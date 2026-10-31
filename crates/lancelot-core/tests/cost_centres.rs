//! Cost centres (R-CC-1, R-BUD-3).

mod common;

use common::*;
use lancelot_core::*;
use proptest::prelude::*;

/// A cost centre with its income and expense categories.
fn drinks_centre(book: &mut Book) -> CostCentreId {
    let centre = book.add_cost_centre("Drinks").unwrap();
    for (kind, name) in [
        (AccountKind::Income, "Drink donations"),
        (AccountKind::Expense, "Drinks"),
        (AccountKind::Expense, "Pfand"),
    ] {
        let category = book.add_account(kind, name).unwrap();
        book.set_category_cost_centre(category, Some(centre))
            .unwrap();
    }
    centre
}

fn tagged(account: AccountId, value: &str, centre: CostCentreId) -> SplitLine {
    SplitLine {
        cost_centre: Some(centre),
        ..split(account, value)
    }
}

fn spend(book: &mut Book, supplier: Option<&str>, lines: Vec<SplitLine>) -> TxnId {
    let payer = asset(book, "Verein account");
    let draft = TxnDraft {
        counterparty: supplier.map(str::to_owned),
        ..draft("Receipt", Entry::Expense { payer, lines })
    };
    book.save_txn(&draft, None).unwrap().id
}

fn earn(book: &mut Book, lines: Vec<SplitLine>) -> TxnId {
    let destination = asset(book, "Cash box");
    let draft = draft("Takings", Entry::Income { destination, lines });
    book.save_txn(&draft, None).unwrap().id
}

fn plan(category: AccountId, estimate: &str, centre: Option<CostCentreId>) -> BudgetLineDraft {
    BudgetLineDraft {
        category,
        description: "Plan".into(),
        estimate: Estimate::Amount {
            amount: eur(estimate),
        },
        complete: false,
        cost_centre: centre,
    }
}

/// The Drinks frame in `UI.fig`: an invoice with Pfand, and ice.
fn the_frame() -> Book {
    let mut book = new_book();
    let centre = drinks_centre(&mut book);
    let (drinks, pfand) = (expense(&book, "Drinks"), expense(&book, "Pfand"));
    spend(
        &mut book,
        Some("Hoffmann"),
        vec![tagged(drinks, "420", centre), tagged(pfand, "32", centre)],
    );
    let ice = book.add_account(AccountKind::Expense, "Ice").unwrap();
    spend(&mut book, Some("Aral"), vec![tagged(ice, "32", centre)]);
    let donations = income(&book, "Drink donations");
    earn(&mut book, vec![tagged(donations, "812,50", centre)]);
    book
}

#[test]
fn the_drinks_result_is_what_is_tagged_to_it() {
    let mut book = the_frame();
    // Not tagged: not in the P&L.
    let kitchen = expense(&book, "Kitchen");
    spend(&mut book, None, vec![split(kitchen, "100")]);

    let centre = book.cost_centres().next().unwrap().id;
    let pnl = book.budget().cost_centre(centre).unwrap();
    assert_eq!(pnl.expense.actual, eur("484"));
    assert_eq!(pnl.income.actual, eur("812,50"));
    assert_eq!(pnl.result.actual, eur("328,50"));
    let names: Vec<_> = pnl
        .categories
        .iter()
        .map(|c| book.account(c.category).unwrap().name.as_str())
        .collect();
    assert_eq!(names, ["Drink donations", "Drinks", "Pfand", "Ice"]);
}

#[test]
fn a_tag_must_exist() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let payer = asset(&book, "Verein account");
    let lines = vec![tagged(kitchen, "1", CostCentreId(99))];
    let err = book
        .save_txn(&draft("x", Entry::Expense { payer, lines }), None)
        .unwrap_err();
    assert_eq!(err.field(), Some("lines[0].costCentre"));
    let err = book
        .add_budget_line(plan(kitchen, "1", Some(CostCentreId(99))))
        .unwrap_err();
    assert_eq!(err.field(), Some("costCentre"));
}

#[test]
fn a_linked_line_has_the_cost_centre_of_its_budget_line() {
    let mut book = new_book();
    let centre = drinks_centre(&mut book);
    let kitchen = expense(&book, "Kitchen");
    let food = book.add_budget_line(plan(kitchen, "100", None)).unwrap();
    let payer = asset(&book, "Verein account");

    let lines = vec![SplitLine {
        budget_line: Some(food),
        ..tagged(kitchen, "10", centre)
    }];
    let err = book
        .save_txn(&draft("x", Entry::Expense { payer, lines }), None)
        .unwrap_err();
    assert_eq!(err.code(), "wrong_cost_centre");

    let lines = vec![SplitLine {
        budget_line: Some(food),
        ..split(kitchen, "10")
    }];
    let txn = spend(&mut book, None, lines);

    // Moving the budget line takes its receipts along, so plan and actual stay together.
    book.update_budget_line(food, plan(kitchen, "100", Some(centre)))
        .unwrap();
    let line = &book.txn(txn).unwrap().lines[1];
    assert_eq!(line.cost_centre, Some(centre));
    let pnl = book.budget().cost_centre(centre).unwrap();
    assert_eq!(pnl.expense.estimate, eur("100"));
    assert_eq!(pnl.expense.actual, eur("10"));
    assert_eq!(pnl.expense.forecast, eur("100"));
    assert_eq!(pnl.categories[0].unbudgeted, Cents::ZERO);
}

#[test]
fn deleting_a_cost_centre_leaves_everything_untagged() {
    let mut book = new_book();
    let centre = book.add_cost_centre(" Kitchen crew ").unwrap();
    assert_eq!(book.cost_centre(centre).unwrap().name, "Kitchen crew");
    let kitchen = expense(&book, "Kitchen");
    book.set_category_cost_centre(kitchen, Some(centre))
        .unwrap();
    let food = book
        .add_budget_line(plan(kitchen, "100", Some(centre)))
        .unwrap();
    let txn = spend(
        &mut book,
        None,
        vec![SplitLine {
            budget_line: Some(food),
            ..tagged(kitchen, "10", centre)
        }],
    );

    book.delete_cost_centre(centre).unwrap();
    assert_eq!(book.cost_centre(centre), None);
    assert_eq!(book.account(kitchen).unwrap().cost_centre, None);
    assert_eq!(book.budget_line(food).unwrap().draft.cost_centre, None);
    assert_eq!(book.txn(txn).unwrap().lines[1].cost_centre, None);
    assert_eq!(book.txn(txn).unwrap().lines[1].budget_line, Some(food));
}

#[test]
fn names_stay_unique_and_a_new_event_has_no_cost_centre() {
    let mut book = new_book();
    assert_eq!(book.cost_centres().count(), 0);
    let centre = drinks_centre(&mut book);
    assert_eq!(
        book.add_cost_centre("drinks").unwrap_err().code(),
        "duplicate_name"
    );
    book.rename_cost_centre(centre, "Getränke").unwrap();
    assert_eq!(book.cost_centre(centre).unwrap().name, "Getränke");
}

#[test]
fn only_categories_start_with_a_cost_centre() {
    let mut book = new_book();
    let centre = drinks_centre(&mut book);
    let verein = asset(&book, "Verein account");
    assert_eq!(
        book.set_category_cost_centre(verein, Some(centre))
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
}

#[test]
fn the_csv_names_the_cost_centre() {
    let book = the_frame();
    let csv = transactions_csv(&book, NumberFormat::De);
    assert!(csv.lines().next().unwrap().contains("Cost centre"));
    // The two invoice lines, the ice and the donations.
    let tagged = csv.lines().filter(|l| l.ends_with(";Drinks")).count();
    assert_eq!(tagged, 4, "{csv}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Tagged parts add up to the whole: with every line and plan tagged, the cost centres' figures
    /// are the event's. Tags never change a balance.
    #[test]
    fn cost_centres_add_up_to_the_budget(
        plans in prop::collection::vec((0usize..10, 0i64..500_000, any::<bool>(), 0usize..2), 0..8),
        entries in prop::collection::vec((0usize..10, 1i64..300_000, 0usize..9, 0usize..2), 0..20),
    ) {
        let mut book = new_book();
        let centres = [drinks_centre(&mut book), book.add_cost_centre("Crew").unwrap()];
        let categories: Vec<AccountId> = book
            .accounts()
            .filter(|a| matches!(a.kind, AccountKind::Income | AccountKind::Expense))
            .map(|a| a.id)
            .collect();
        let mut lines = Vec::new();
        for (c, cents, complete, centre) in &plans {
            let category = categories[c % categories.len()];
            let cost_centre = Some(centres[*centre]);
            let id = book
                .add_budget_line(BudgetLineDraft {
                    category,
                    description: "Plan".into(),
                    estimate: Estimate::Amount { amount: Cents::new(*cents).unwrap() },
                    complete: *complete,
                    cost_centre,
                })
                .unwrap();
            lines.push((id, category, cost_centre));
        }
        for (c, cents, link, centre) in &entries {
            let category = categories[c % categories.len()];
            let linked = lines.get(*link).filter(|(_, cat, _)| *cat == category);
            let line = SplitLine {
                account: category,
                amount: Cents::new(*cents).unwrap(),
                budget_line: linked.map(|(id, _, _)| *id),
                cost_centre: linked.map_or(Some(centres[*centre]), |(_, _, c)| *c),
            };
            if book.account(category).unwrap().kind == AccountKind::Income {
                earn(&mut book, vec![line]);
            } else {
                spend(&mut book, None, vec![line]);
            }
        }

        let budget = book.budget();
        let total = |f: fn(&CostCentreFigures) -> Figures| {
            budget.cost_centres.iter().map(f).fold(Figures::default(), |a, b| Figures {
                estimate: a.estimate.checked_add(b.estimate).unwrap(),
                actual: a.actual.checked_add(b.actual).unwrap(),
                forecast: a.forecast.checked_add(b.forecast).unwrap(),
            })
        };
        prop_assert_eq!(total(|c| c.income), budget.income);
        prop_assert_eq!(total(|c| c.expense), budget.expense);
        prop_assert_eq!(total(|c| c.result), budget.result);
        let raw: i64 = book.accounts().map(|a| {
            book.ledger().balance(a.id).get() * a.kind.display_sign()
        }).sum();
        prop_assert_eq!(raw, 0);
    }
}
