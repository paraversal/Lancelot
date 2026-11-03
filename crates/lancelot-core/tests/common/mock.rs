//! The example event from the design mockups (UI.fig), built from individual transactions.
//! Only the totals come from the mockups; the transactions behind them are made up.

use lancelot_core::*;
use serde_json::{Value, json};

use super::{asset, draft, eur, expense, income, june, split};

/// Something the example event can be built in: a [`Book`] or a [`Store`].
pub trait Target {
    fn book(&self) -> &Book;
    fn person(&mut self, name: &str) -> AccountId;
    fn save(&mut self, description: &str, entry: Entry) -> TxnId;
    /// A made-up receipt: a photo, or a PDF for invoices.
    fn attach(&mut self, txn: TxnId, pdf: bool);
    fn count(&mut self, account: AccountId, amount: Cents);
    fn checkpoint(&mut self, account: AccountId, date: time::Date, amount: Cents);
    fn budget(&mut self, draft: BudgetLineDraft) -> BudgetLineId;
    fn save_draft(&mut self, draft: TxnDraft) -> TxnId;
    fn edit<R>(&mut self, command: impl FnOnce(&mut Book) -> Result<R, DomainError>) -> R;
}

impl Target for Book {
    fn book(&self) -> &Book {
        self
    }
    fn person(&mut self, name: &str) -> AccountId {
        self.add_person(name, Person::default()).unwrap()
    }
    fn save(&mut self, description: &str, entry: Entry) -> TxnId {
        self.save_txn(&draft(description, entry), None).unwrap().id
    }
    fn attach(&mut self, txn: TxnId, pdf: bool) {
        if pdf {
            self.add_attachment(
                txn,
                "invoice.pdf",
                "application/pdf",
                2048,
                "00",
                super::now(),
            )
            .unwrap();
        } else {
            super::attach(self, txn);
        }
    }
    fn count(&mut self, account: AccountId, amount: Cents) {
        self.set_closing_count(account, june(14), amount).unwrap();
    }
    fn checkpoint(&mut self, account: AccountId, date: time::Date, amount: Cents) {
        self.add_reconciliation(account, date, amount).unwrap();
    }
    fn budget(&mut self, draft: BudgetLineDraft) -> BudgetLineId {
        self.add_budget_line(draft).unwrap()
    }
    fn save_draft(&mut self, draft: TxnDraft) -> TxnId {
        self.save_txn(&draft, None).unwrap().id
    }
    fn edit<R>(&mut self, command: impl FnOnce(&mut Book) -> Result<R, DomainError>) -> R {
        command(self).unwrap()
    }
}

impl Target for Store {
    fn book(&self) -> &Book {
        Store::book(self)
    }
    fn person(&mut self, name: &str) -> AccountId {
        self.change(|b| b.add_person(name, Person::default()))
            .unwrap()
    }
    fn save(&mut self, description: &str, entry: Entry) -> TxnId {
        self.change(|b| b.save_txn(&draft(description, entry), None))
            .unwrap()
            .id
    }
    fn attach(&mut self, txn: TxnId, pdf: bool) {
        let (name, bytes): (&str, &[u8]) = if pdf {
            (
                "invoice.pdf",
                include_bytes!("../../../../fixtures/receipts/invoice.pdf"),
            )
        } else {
            (
                "receipt.jpg",
                include_bytes!("../../../../fixtures/receipts/receipt.jpg"),
            )
        };
        self.add_attachment(txn, name, bytes).unwrap();
    }
    fn count(&mut self, account: AccountId, amount: Cents) {
        self.change(|b| b.set_closing_count(account, june(14), amount))
            .unwrap();
    }
    fn checkpoint(&mut self, account: AccountId, date: time::Date, amount: Cents) {
        self.change(|b| b.add_reconciliation(account, date, amount))
            .unwrap();
    }
    fn budget(&mut self, draft: BudgetLineDraft) -> BudgetLineId {
        self.change(|b| b.add_budget_line(draft)).unwrap()
    }
    fn save_draft(&mut self, draft: TxnDraft) -> TxnId {
        self.change(|b| b.save_txn(&draft, None)).unwrap().id
    }
    fn edit<R>(&mut self, command: impl FnOnce(&mut Book) -> Result<R, DomainError>) -> R {
        self.change(command).unwrap()
    }
}

pub struct Mock {
    pub anna: AccountId,
    pub malte: AccountId,
    pub bob: AccountId,
    pub clara: AccountId,
    /// The one expense without a receipt.
    pub cups: TxnId,
}

pub fn build(t: &mut impl Target) -> Mock {
    // What the drinks plugin's setup creates.
    let (centre, donations, drinks, deposit) = t.edit(|b| {
        let centre = b.add_cost_centre("Drinks")?;
        let donations = b.add_account(AccountKind::Income, "Drink donations")?;
        let drinks = b.add_account(AccountKind::Expense, "Drinks")?;
        let deposit = b.add_account(AccountKind::Expense, "Deposit")?;
        for category in [donations, drinks, deposit] {
            b.set_category_cost_centre(category, Some(centre))?;
        }
        Ok((centre, donations, drinks, deposit))
    });
    let b = t.book();
    let (bank, cash) = (asset(b, "Bank account"), asset(b, "Cash box"));
    let [tickets, other_in] = ["Tickets", "General"].map(|n| income(b, n));
    let [food, venue, tech, other, mobility] =
        ["Kitchen", "Venue", "Tech", "General", "Mobility"].map(|n| expense(b, n));
    let tag = move |account| {
        [donations, drinks, deposit]
            .contains(&account)
            .then_some(centre)
    };
    let [anna, malte, bob, clara] = ["Anna", "Malte", "Bob", "Clara"].map(|n| t.person(n));

    // The budget. The mockups have none, so it is made up: some lines over, some under, some
    // complete, and a few receipts left unbudgeted (Deposit, cups, cables, decoration).
    let mut plan = |category, description: &str, estimate, complete| {
        t.budget(BudgetLineDraft {
            cost_centre: tag(category),
            category,
            description: description.into(),
            estimate,
            complete,
        })
    };
    let amount = |text| Estimate::Amount { amount: eur(text) };
    let times = |quantity, text| Estimate::Quantity {
        quantity,
        unit_price: eur(text),
    };
    let b_tickets = plan(tickets, "Ticket sales", times(260, "25"), true);
    let b_bar = plan(donations, "Bar donations", amount("700"), false);
    let b_hall = plan(venue, "Hall rent", amount("1.800"), true);
    let b_catering = plan(food, "Catering", amount("1.100"), true);
    let b_groceries = plan(food, "Groceries", times(3, "250"), false);
    let b_drinks = plan(drinks, "Drink crates", times(30, "14"), false);
    let b_pa = plan(tech, "PA rental", amount("450"), true);
    let b_insurance = plan(other, "Insurance", amount("225"), true);
    plan(mobility, "Van rental", amount("150"), false);

    let to = |line| {
        move |account, amount: &str| SplitLine {
            budget_line: Some(line),
            cost_centre: tag(account),
            ..split(account, amount)
        }
    };
    let income_to = |destination, line: SplitLine| Entry::Income {
        destination,
        lines: vec![line],
    };
    let expense_by = |payer, lines: Vec<SplitLine>| Entry::Expense { payer, lines };
    let settle = |person, amount: &str, direction| Entry::Settle {
        person,
        account: bank,
        amount: eur(amount),
        direction,
    };

    // Income
    t.save(
        "Ticket sheet",
        income_to(bank, to(b_tickets)(tickets, "6.227,50")),
    );
    t.save(
        "Tickets, paid to Bob",
        income_to(bob, to(b_tickets)(tickets, "300")),
    );
    t.save(
        "Drink donations",
        income_to(cash, to(b_bar)(donations, "812,50")),
    );
    t.save("Donation box", income_to(cash, split(other_in, "50")));

    // The drinks plugin's records: the supplier's invoice checks against them.
    let mut records = vec![PluginChange {
        key: format!("centre:{}", centre.0),
        scope: PluginScope::Setup,
        value: json!({ "supplier": "Hoffmann", "deposit": deposit.0 }),
    }];
    for (id, (name, taken, returned, left_over, logged, cost, donation)) in [
        ("Club-Mate 0,5", 240, 96, 4, 138, 115, 250),
        ("Lager 0,33", 480, 312, 0, 165, 95, 200),
        ("Shandy 0,33", 120, 72, 2, 45, 95, 200),
        ("Apple spritzer 1,0", 72, 48, 2, 22, 140, 300),
        ("Water 1,0", 96, 70, 6, 20, 60, 100),
    ]
    .into_iter()
    .enumerate()
    {
        records.push(PluginChange {
            key: format!("product:{}", id + 1),
            scope: PluginScope::Setup,
            value: json!({ "centre": centre.0, "name": name, "cost": cost, "donation": donation }),
        });
        records.push(PluginChange {
            key: format!("count:{}", id + 1),
            scope: PluginScope::Records,
            value: json!({
                "taken": taken,
                "returned": returned,
                "leftOver": left_over,
                "manual": logged,
                "imported": Value::Null,
            }),
        });
    }
    t.edit(|b| b.apply_plugin_changes("drinks".into(), records));
    let invoice = t.save_draft(TxnDraft {
        counterparty: Some("Hoffmann".into()),
        ..draft(
            "Hoffmann drinks",
            expense_by(
                bank,
                vec![
                    to(b_drinks)(drinks, "420"),
                    SplitLine {
                        cost_centre: tag(deposit),
                        ..split(deposit, "32")
                    },
                ],
            ),
        )
    });
    t.attach(invoice, false);

    // Expenses with receipts
    for (description, payer, lines) in [
        ("Hall rent", bank, vec![to(b_hall)(venue, "1.800")]),
        ("PA rental", bank, vec![to(b_pa)(tech, "500")]),
        ("Catering", bank, vec![to(b_catering)(food, "1.098,50")]),
        ("Event insurance", bank, vec![to(b_insurance)(other, "225")]),
        (
            "Wholesale market",
            cash,
            vec![to(b_groceries)(food, "342,75"), split(other, "48,50")],
        ),
        ("Bakery", cash, vec![to(b_groceries)(food, "226,35")]),
        ("Groceries", anna, vec![to(b_groceries)(food, "312,40")]),
        ("Cables", malte, vec![split(tech, "110")]),
        ("Snacks", bob, vec![to(b_groceries)(food, "120")]),
        ("Decoration", clara, vec![split(other, "95")]),
    ] {
        let invoice = matches!(description, "Hall rent" | "PA rental");
        let id = t.save(description, expense_by(payer, lines));
        t.attach(id, invoice);
    }
    let cups = t.save(
        "Cups and ice",
        expense_by(
            cash,
            vec![SplitLine {
                cost_centre: tag(drinks),
                ..split(drinks, "32")
            }],
        ),
    );

    // Advances and settling
    t.save(
        "Change money",
        Entry::Transfer {
            from: anna,
            to: cash,
            amount: eur("200"),
        },
    );
    t.save(
        "Reimbursement Anna",
        settle(anna, "300", SettleDirection::Reimburse),
    );
    t.save(
        "Reimbursement Clara",
        settle(clara, "95", SettleDirection::Reimburse),
    );

    // Counted 410,00 € against 412,90 € in the books.
    t.count(cash, eur("410"));

    // The Bank account's statement agrees with the books.
    let bank = asset(t.book(), "Bank account");
    let movement = t.book().ledger().balance(bank);
    t.checkpoint(bank, june(30), movement);

    Mock {
        anna,
        malte,
        bob,
        clara,
        cups,
    }
}
