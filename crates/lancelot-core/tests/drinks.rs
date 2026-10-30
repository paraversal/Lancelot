//! Cost centres and Drinks (R-CC-1, R-CC-2, R-BUD-3, R-DRK-1 to R-DRK-6).

mod common;

use common::*;
use lancelot_core::*;
use proptest::prelude::*;
use tempfile::TempDir;

fn drinks_centre(book: &Book) -> CostCentreId {
    book.drinks_cost_centre().unwrap().id
}

fn tagged(account: AccountId, value: &str, centre: CostCentreId) -> SplitLine {
    SplitLine {
        cost_centre: Some(centre),
        ..split(account, value)
    }
}

fn spend(book: &mut Book, supplier: Option<&str>, lines: Vec<SplitLine>) -> TxnId {
    let payer = role(book, Role::Verein);
    let draft = TxnDraft {
        counterparty: supplier.map(str::to_owned),
        ..draft("Receipt", Entry::Expense { payer, lines })
    };
    book.save_txn(&draft, None).unwrap().id
}

fn earn(book: &mut Book, lines: Vec<SplitLine>) -> TxnId {
    let destination = role(book, Role::CashBox);
    let draft = draft("Takings", Entry::Income { destination, lines });
    book.save_txn(&draft, None).unwrap().id
}

fn product(name: &str, taken: u32, returned: u32, unit: &str) -> DrinkProductDraft {
    DrinkProductDraft {
        name: name.into(),
        taken,
        returned,
        unit_price: eur(unit),
    }
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

/// The Drinks frame in `UI.fig`: five products, Hoffmann's invoice with Pfand, and ice.
fn the_frame() -> Book {
    let mut book = new_book();
    let centre = drinks_centre(&book);
    for (name, taken, returned, unit) in [
        ("Club-Mate 0,5", 240, 96, "1,15"),
        ("Pils 0,33", 480, 312, "0,95"),
        ("Radler 0,33", 120, 72, "0,95"),
        ("Apfelschorle 1,0", 72, 48, "1,40"),
        ("Water 1,0", 96, 70, "0,60"),
    ] {
        book.add_drink_product(product(name, taken, returned, unit))
            .unwrap();
    }
    book.set_drink_settings(DrinkSettings {
        supplier: Some(" Hoffmann ".into()),
        price_per_strich: Some(eur("1,00")),
    })
    .unwrap();
    let (drinks, pfand) = (expense(&book, "Drinks"), expense(&book, "Pfand"));
    spend(
        &mut book,
        Some("hoffmann"),
        vec![tagged(drinks, "420", centre), tagged(pfand, "32", centre)],
    );
    let ice = book.add_account(AccountKind::Expense, "Ice").unwrap();
    spend(&mut book, Some("Aral"), vec![tagged(ice, "32", centre)]);
    let donations = income(&book, "Drink donations");
    earn(&mut book, vec![tagged(donations, "812,50", centre)]);
    book
}

#[test]
fn a_new_event_has_the_drinks_cost_centre_and_its_categories() {
    let book = new_book();
    let centre = drinks_centre(&book);
    assert_eq!(book.cost_centre(centre).unwrap().name, "Drinks");
    for category in [
        expense(&book, "Drinks"),
        expense(&book, "Pfand"),
        income(&book, "Drink donations"),
    ] {
        assert_eq!(book.account(category).unwrap().cost_centre, Some(centre));
    }
    assert_eq!(
        book.account(expense(&book, "Kitchen")).unwrap().cost_centre,
        None
    );
    assert_eq!(book.account_by_role(Role::Pfand).unwrap().name, "Pfand");
}

#[test]
fn the_drinks_result_is_what_is_tagged_to_it() {
    let mut book = the_frame();
    // Not tagged: not in the P&L.
    let kitchen = expense(&book, "Kitchen");
    spend(&mut book, None, vec![split(kitchen, "100")]);

    let centre = drinks_centre(&book);
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
fn the_records_check_the_invoice() {
    let book = the_frame();
    let drinks = book.drinks();
    assert_eq!(drinks.kept, 410);
    assert_eq!(drinks.cost, eur("420"));
    assert_eq!(drinks.cost_per_drink, Some(eur("1,02")));
    assert_eq!(
        drinks.invoice,
        Some(InvoiceCheck {
            amount: eur("452"),
            pfand: eur("32"),
            difference: Cents::ZERO,
        })
    );
    let first = book.drink_products().next().unwrap();
    let figures = drinks.product(first.id).unwrap();
    assert_eq!((figures.kept, figures.cost), (144, eur("165,60")));
}

#[test]
fn an_invoice_that_differs_shows_by_how_much() {
    let mut book = the_frame();
    let id = book.drink_products().next().unwrap().id;
    // Ten more returned than the invoice charged for: it charged too much.
    book.update_drink_product(id, product("Club-Mate 0,5", 240, 106, "1,15"))
        .unwrap();
    assert_eq!(book.drinks().invoice.unwrap().difference, eur("11,50"));
}

#[test]
fn only_the_suppliers_tagged_expenses_are_the_invoice() {
    let mut book = new_book();
    let centre = drinks_centre(&book);
    let drinks = expense(&book, "Drinks");
    spend(
        &mut book,
        Some("Hoffmann"),
        vec![tagged(drinks, "10", centre)],
    );
    assert_eq!(book.drinks().invoice, None, "no supplier set");

    book.set_drink_settings(DrinkSettings {
        supplier: Some("Hoffmann".into()),
        price_per_strich: None,
    })
    .unwrap();
    assert_eq!(book.drinks().invoice.unwrap().amount, eur("10"));

    spend(&mut book, Some("Hoffmann"), vec![split(drinks, "5")]);
    spend(&mut book, Some("Aral"), vec![tagged(drinks, "7", centre)]);
    assert_eq!(book.drinks().invoice.unwrap().amount, eur("10"));
}

#[test]
fn products_are_checked() {
    let mut book = new_book();
    let err = book
        .add_drink_product(product("Pils", 10, 11, "1"))
        .unwrap_err();
    assert_eq!(err.code(), "too_many_returned");
    assert_eq!(err.field(), Some("returned"));
    let err = book.add_drink_product(product(" ", 1, 0, "1")).unwrap_err();
    assert_eq!(err.field(), Some("name"));
    let err = book
        .add_drink_product(product("Pils", 1, 0, "-1"))
        .unwrap_err();
    assert_eq!(err.field(), Some("unitPrice"));
    assert_eq!(book.drinks().cost_per_drink, None);

    book.add_drink_product(product("Pils", 5, 5, "1")).unwrap();
    assert_eq!(book.drinks().cost_per_drink, None, "nothing kept");
}

#[test]
fn a_tag_must_exist() {
    let mut book = new_book();
    let kitchen = expense(&book, "Kitchen");
    let payer = role(&book, Role::Verein);
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
    let centre = drinks_centre(&book);
    let kitchen = expense(&book, "Kitchen");
    let food = book.add_budget_line(plan(kitchen, "100", None)).unwrap();
    let payer = role(&book, Role::Verein);

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
fn drinks_stays_and_names_stay_unique() {
    let mut book = new_book();
    let centre = drinks_centre(&book);
    assert_eq!(
        book.delete_cost_centre(centre).unwrap_err().code(),
        "cost_centre_protected"
    );
    assert_eq!(
        book.add_cost_centre("drinks").unwrap_err().code(),
        "duplicate_name"
    );
    book.rename_cost_centre(centre, "Getränke").unwrap();
    assert_eq!(book.drinks_cost_centre().unwrap().name, "Getränke");
}

#[test]
fn only_categories_start_with_a_cost_centre() {
    let mut book = new_book();
    let centre = drinks_centre(&book);
    let verein = role(&book, Role::Verein);
    assert_eq!(
        book.set_category_cost_centre(verein, Some(centre))
            .unwrap_err()
            .code(),
        "wrong_account_kind"
    );
}

#[test]
fn everything_survives_saving_and_reopening() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("event.lancelot");
    let details = new_book().event().details.clone();
    let (mut store, _) = Store::create(&path, details, StoreOptions::default()).unwrap();

    let centre = store.book().drinks_cost_centre().unwrap().id;
    let crew = store.change(|book| book.add_cost_centre("Crew")).unwrap();
    let gone = store.change(|book| book.add_cost_centre("Gone")).unwrap();
    let kitchen = expense(store.book(), "Kitchen");
    let verein = role(store.book(), Role::Verein);
    store
        .change(|book| book.set_category_cost_centre(kitchen, Some(crew)))
        .unwrap();
    let food = store
        .change(|book| book.add_budget_line(plan(kitchen, "100", Some(crew))))
        .unwrap();
    let pils = store
        .change(|book| book.add_drink_product(product("Pils", 10, 4, "1")))
        .unwrap();
    let cola = store
        .change(|book| book.add_drink_product(product("Cola", 5, 1, "2")))
        .unwrap();
    store
        .change(|book| book.update_drink_product(pils, product("Pils 0,33", 10, 5, "1")))
        .unwrap();
    store
        .change(|book| book.delete_drink_product(cola))
        .unwrap();
    store
        .change(|book| {
            book.set_drink_settings(DrinkSettings {
                supplier: Some("Hoffmann".into()),
                price_per_strich: Some(eur("1,50")),
            })
        })
        .unwrap();
    let drinks = expense(store.book(), "Drinks");
    let lines = vec![
        tagged(drinks, "5", centre),
        SplitLine {
            budget_line: Some(food),
            ..tagged(kitchen, "20", crew)
        },
    ];
    let txn = TxnDraft {
        counterparty: Some("Hoffmann".into()),
        ..draft(
            "Invoice",
            Entry::Expense {
                payer: verein,
                lines,
            },
        )
    };
    store.change(|book| book.save_txn(&txn, None)).unwrap();
    // A centre that goes away must clear its references before its row does.
    let lines = vec![tagged(kitchen, "1", gone)];
    let last = draft(
        "Other",
        Entry::Expense {
            payer: verein,
            lines,
        },
    );
    store.change(|book| book.save_txn(&last, None)).unwrap();
    store.change(|book| book.delete_cost_centre(gone)).unwrap();

    let entities: Vec<String> = store
        .change_log(100)
        .unwrap()
        .into_iter()
        .map(|e| e.entity)
        .collect();
    assert!(entities.iter().any(|e| e == "cost_centre"));
    assert!(entities.iter().any(|e| e == "drink_product"));

    let before = store.book().clone();
    drop(store);
    let (store, report) = Store::open(&path, StoreOptions::default()).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(store.book(), &before);
    assert_eq!(
        store.book().drinks().invoice.unwrap().difference,
        Cents::ZERO
    );
    assert_eq!(
        store.book().event().drinks.price_per_strich,
        Some(eur("1,50"))
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
        let centres = [drinks_centre(&book), book.add_cost_centre("Crew").unwrap()];
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
                memo: None,
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

    /// The cost of the records is what is kept at its price, and the invoice check is exact.
    #[test]
    fn the_records_add_up(
        products in prop::collection::vec((0u32..1_000, 0u32..1_000, 0i64..2_000), 0..8),
        extra in -5_000i64..5_000,
    ) {
        let mut book = new_book();
        let centre = drinks_centre(&book);
        book.set_drink_settings(DrinkSettings {
            supplier: Some("Hoffmann".into()),
            price_per_strich: None,
        }).unwrap();
        let mut cost = 0i64;
        for (taken, returned, unit) in &products {
            let returned = (*returned).min(*taken);
            book.add_drink_product(DrinkProductDraft {
                name: "Product".into(),
                taken: *taken,
                returned,
                unit_price: Cents::new(*unit).unwrap(),
            }).unwrap();
            cost += i64::from(taken - returned) * unit;
        }
        prop_assert_eq!(book.drinks().cost.get(), cost);
        let invoiced = cost + extra;
        if invoiced > 0 {
            let drinks = expense(&book, "Drinks");
            spend(&mut book, Some("Hoffmann"), vec![SplitLine {
                amount: Cents::new(invoiced).unwrap(),
                ..tagged(drinks, "1", centre)
            }]);
            prop_assert_eq!(book.drinks().invoice.unwrap().difference.get(), extra);
        } else {
            prop_assert_eq!(book.drinks().invoice, None);
        }
    }
}
