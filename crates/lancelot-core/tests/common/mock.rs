//! The example event from the design mockups (UI.fig), built from individual transactions.
//! Only the totals come from the mockups; the transactions behind them are made up.

use lancelot_core::*;

use super::{draft, eur, expense, income, june, role, split};

/// Something the example event can be built in: a [`Book`] or a [`Store`].
pub trait Target {
    fn book(&self) -> &Book;
    fn person(&mut self, name: &str) -> AccountId;
    fn save(&mut self, description: &str, entry: Entry) -> TxnId;
    /// A made-up receipt: a photo, or a PDF for invoices.
    fn attach(&mut self, txn: TxnId, pdf: bool);
    fn count(&mut self, account: AccountId, amount: Cents);
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
}

impl Target for Store {
    fn book(&self) -> &Book {
        Store::book(self)
    }
    fn person(&mut self, name: &str) -> AccountId {
        self.add_person(name, Person::default()).unwrap()
    }
    fn save(&mut self, description: &str, entry: Entry) -> TxnId {
        self.save_txn(&draft(description, entry), None).unwrap().id
    }
    fn attach(&mut self, txn: TxnId, pdf: bool) {
        let (name, bytes): (&str, &[u8]) = if pdf {
            ("invoice.pdf", b"%PDF-1.7\n an invoice")
        } else {
            ("receipt.jpg", b"\xFF\xD8\xFF\xE0 a receipt")
        };
        self.add_attachment(txn, name, bytes).unwrap();
    }
    fn count(&mut self, account: AccountId, amount: Cents) {
        self.set_closing_count(account, june(14), amount).unwrap();
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
    let b = t.book();
    let (verein, cash) = (role(b, Role::Verein), role(b, Role::CashBox));
    let [tickets, donations, other_in] =
        ["Tickets", "Drink donations", "General"].map(|n| income(b, n));
    let [food, venue, drinks, pfand, tech, other] =
        ["Kitchen", "Venue", "Drinks", "Pfand", "Tech", "General"].map(|n| expense(b, n));
    let [anna, malte, bob, clara] = ["Anna", "Malte", "Bob", "Clara"].map(|n| t.person(n));

    let income_to = |destination, account, amount: &str| Entry::Income {
        destination,
        lines: vec![split(account, amount)],
    };
    let expense_by = |payer, lines: Vec<SplitLine>| Entry::Expense { payer, lines };
    let settle = |person, amount: &str, direction| Entry::Settle {
        person,
        account: verein,
        amount: eur(amount),
        direction,
    };

    // Income
    t.save("Ticket sheet", income_to(verein, tickets, "6.227,50"));
    t.save("Tickets, paid to Bob", income_to(bob, tickets, "300"));
    t.save("Drink donations", income_to(cash, donations, "812,50"));
    t.save("Donation box", income_to(cash, other_in, "50"));

    // Expenses with receipts
    for (description, payer, lines) in [
        ("Hall rent", verein, vec![split(venue, "1.800")]),
        (
            "Hoffmann drinks",
            verein,
            vec![split(drinks, "420"), split(pfand, "32")],
        ),
        ("PA rental", verein, vec![split(tech, "500")]),
        ("Catering", verein, vec![split(food, "1.098,50")]),
        ("Event insurance", verein, vec![split(other, "225")]),
        (
            "Großmarkt",
            cash,
            vec![split(food, "342,75"), split(other, "48,50")],
        ),
        ("Bakery", cash, vec![split(food, "226,35")]),
        ("Groceries", anna, vec![split(food, "312,40")]),
        ("Cables", malte, vec![split(tech, "110")]),
        ("Snacks", bob, vec![split(food, "120")]),
        ("Decoration", clara, vec![split(other, "95")]),
    ] {
        let invoice = matches!(description, "Hall rent" | "PA rental");
        let id = t.save(description, expense_by(payer, lines));
        t.attach(id, invoice);
    }
    let cups = t.save("Cups and ice", expense_by(cash, vec![split(drinks, "32")]));

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

    Mock {
        anna,
        malte,
        bob,
        clara,
        cups,
    }
}
