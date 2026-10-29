//! The event in memory: its details, accounts, persons and transactions, plus the derived
//! [`Ledger`]. Every change is applied to a copy, checked, and only then kept, so a `Book`
//! is always consistent. [`crate::store::Store`] loads a `Book` and writes each accepted change.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use time::{Date, OffsetDateTime};
use ts_rs::TS;

use crate::budget::Budget;
use crate::entry::{Entry, SettleDirection, TxnDraft, build_lines, lines_follow_rules, txn_amount};
use crate::error::DomainError;
use crate::iban::validate_bic;
use crate::ledger::{BlockerOverride, Ledger, LedgerInput};
use crate::model::{
    Account, AccountId, AccountKind, AssetType, Attachment, AttachmentId, BudgetLine,
    BudgetLineDraft, BudgetLineId, ClosingCount, Estimate, Event, EventDetails, Line, Person, Role,
    Txn, TxnId, TxnKind,
};
use crate::money::Cents;

/// The largest attachment Lancelot accepts. Phone photos and PDFs are far smaller; the limit
/// only catches accidents.
pub const MAX_ATTACHMENT_SIZE: u64 = 50 * 1024 * 1024;

/// Default categories of a new event (R-ACC-2).
const DEFAULT_INCOME: &[&str] = &["Tickets", "Drink donations", "General"];
const DEFAULT_EXPENSE: &[&str] = &[
    "Kitchen", "Drinks", "Pfand", "Mobility", "Venue", "Tech", "General",
];

/// The description of an opening transaction made by [`Book::set_opening_count`].
const OPENING_DESCRIPTION: &str = "Opening count";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    event: Event,
    accounts: BTreeMap<AccountId, Account>,
    persons: BTreeMap<AccountId, Person>,
    txns: BTreeMap<TxnId, Txn>,
    attachments: BTreeMap<AttachmentId, Attachment>,
    closing_counts: BTreeMap<AccountId, ClosingCount>,
    budget_lines: BTreeMap<BudgetLineId, BudgetLine>,
    next: NextIds,
    ledger: Ledger,
    budget: Budget,
}

/// The ids the next new account, transaction, attachment and budget line get. Ids are never reused, so
/// the change log stays unambiguous after deletions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextIds {
    pub account: i64,
    pub txn: i64,
    pub attachment: i64,
    pub budget_line: i64,
}

/// A book taken apart into its stored pieces. [`Book::from_parts`] checks them all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookParts {
    pub event: Event,
    pub accounts: Vec<Account>,
    pub persons: Vec<(AccountId, Person)>,
    pub txns: Vec<Txn>,
    pub attachments: Vec<Attachment>,
    pub closing_counts: Vec<ClosingCount>,
    pub budget_lines: Vec<BudgetLine>,
    pub next: NextIds,
}

/// The outcome of saving a transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Saved {
    pub id: TxnId,
    pub warnings: Vec<Warning>,
}

/// Accepted, but worth pointing out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Warning {
    /// The settle pays out more than the person is owed, or takes back more than they owe.
    SettleExceedsBalance { person: AccountId, net_after: Cents },
}

impl Book {
    /// A new event with the default accounts and categories (R-EVT-1, R-ACC-1, R-ACC-2).
    pub fn new(details: EventDetails) -> Result<Book, DomainError> {
        let details = clean_details(details)?;
        let mut book = Book {
            event: Event {
                details,
                closed_at: None,
                default_reimbursement_account: AccountId(1),
                closing_overrides: Vec::new(),
            },
            accounts: BTreeMap::new(),
            persons: BTreeMap::new(),
            txns: BTreeMap::new(),
            attachments: BTreeMap::new(),
            closing_counts: BTreeMap::new(),
            budget_lines: BTreeMap::new(),
            next: NextIds {
                account: 1,
                txn: 1,
                attachment: 1,
                budget_line: 1,
            },
            ledger: Ledger::default(),
            budget: Budget::default(),
        };
        let verein = book.insert_account(AccountKind::Asset, "Verein account", Some(Role::Verein));
        book.insert_account(AccountKind::Asset, "Cash box", Some(Role::CashBox));
        book.insert_account(AccountKind::Opening, "Opening balance", Some(Role::Opening));
        for name in DEFAULT_INCOME {
            book.insert_account(AccountKind::Income, name, None);
        }
        for name in DEFAULT_EXPENSE {
            book.insert_account(AccountKind::Expense, name, None);
        }
        book.event.default_reimbursement_account = verein;
        book.recompute()?;
        Ok(book)
    }

    /// Rebuilds a book from stored pieces, checking everything a hand-edited or damaged
    /// file could get wrong: roles, kinds, names, transaction shapes, references and ids.
    pub fn from_parts(parts: BookParts) -> Result<Book, DomainError> {
        let bad = |reason: &str| DomainError::Inconsistent {
            reason: reason.to_owned(),
        };

        let mut accounts = BTreeMap::new();
        for a in parts.accounts {
            if a.name.trim().is_empty() || a.id.0 >= parts.next.account {
                return Err(bad(
                    "account with an empty name or an id beyond the next id",
                ));
            }
            accounts.insert(a.id, a);
        }
        for role in [Role::Verein, Role::CashBox, Role::Opening] {
            if accounts.values().filter(|a| a.role == Some(role)).count() != 1 {
                return Err(bad("each system account must exist exactly once"));
            }
        }
        let opening_ok = accounts
            .values()
            .all(|a| (a.kind == AccountKind::Opening) == (a.role == Some(Role::Opening)));
        let roles_ok = accounts.values().all(|a| match a.role {
            Some(Role::Verein | Role::CashBox) => a.kind == AccountKind::Asset,
            _ => true,
        });
        let types_ok = accounts
            .values()
            .all(|a| a.asset_type.is_some() == (a.kind == AccountKind::Asset));
        let names: BTreeSet<(AccountKind, String)> = accounts
            .values()
            .map(|a| (a.kind, a.name.to_ascii_lowercase()))
            .collect();
        if !opening_ok || !roles_ok || !types_ok || names.len() != accounts.len() {
            return Err(bad("account kinds, roles or names are inconsistent"));
        }

        let mut persons = BTreeMap::new();
        for (id, p) in parts.persons {
            if accounts.get(&id).map(|a| a.kind) != Some(AccountKind::Person) {
                return Err(bad("person details for an account that is not a person"));
            }
            persons.insert(id, p);
        }
        if accounts
            .values()
            .any(|a| a.kind == AccountKind::Person && !persons.contains_key(&a.id))
        {
            return Err(bad("person account without person details"));
        }

        let mut budget_lines = BTreeMap::new();
        for b in parts.budget_lines {
            if b.id.0 >= parts.next.budget_line
                || clean_budget_line(b.draft.clone(), &accounts).as_ref() != Ok(&b.draft)
            {
                return Err(bad("budget line with an invalid category or estimate"));
            }
            budget_lines.insert(b.id, b);
        }

        let mut txns = BTreeMap::new();
        for t in parts.txns {
            if t.id.0 >= parts.next.txn
                || !lines_follow_rules(t.kind, &t.lines, &accounts)
                || !links_valid(&t.lines, &budget_lines)
            {
                return Err(DomainError::MalformedTxn { txn: t.id });
            }
            txns.insert(t.id, t);
        }

        let mut attachments = BTreeMap::new();
        for a in parts.attachments {
            if !txns.contains_key(&a.txn) || a.id.0 >= parts.next.attachment {
                return Err(bad("attachment of an unknown transaction"));
            }
            attachments.insert(a.id, a);
        }

        let mut closing_counts = BTreeMap::new();
        for c in parts.closing_counts {
            if accounts.get(&c.account).map(|a| a.kind) != Some(AccountKind::Asset)
                || c.amount.is_negative()
            {
                return Err(bad("closing count on an account that is not an asset"));
            }
            closing_counts.insert(c.account, c);
        }

        let default = parts.event.default_reimbursement_account;
        if accounts.get(&default).map(|a| a.kind) != Some(AccountKind::Asset) {
            return Err(bad("default reimbursement account is not an asset"));
        }

        let mut book = Book {
            event: parts.event,
            accounts,
            persons,
            txns,
            attachments,
            closing_counts,
            budget_lines,
            next: parts.next,
            ledger: Ledger::default(),
            budget: Budget::default(),
        };
        book.recompute()?;
        Ok(book)
    }

    // --- reading -------------------------------------------------------------------------

    pub fn event(&self) -> &Event {
        &self.event
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    pub fn account(&self, id: AccountId) -> Option<&Account> {
        self.accounts.get(&id)
    }

    pub fn account_by_role(&self, role: Role) -> Option<&Account> {
        self.accounts.values().find(|a| a.role == Some(role))
    }

    /// Looks up an account by kind and name, ignoring case.
    pub fn account_by_name(&self, kind: AccountKind, name: &str) -> Option<&Account> {
        self.accounts
            .values()
            .find(|a| a.kind == kind && a.name.eq_ignore_ascii_case(name.trim()))
    }

    pub fn person(&self, id: AccountId) -> Option<&Person> {
        self.persons.get(&id)
    }

    pub fn persons(&self) -> impl Iterator<Item = (&Account, &Person)> {
        self.persons
            .iter()
            .filter_map(|(id, p)| self.accounts.get(id).map(|a| (a, p)))
    }

    pub fn txns(&self) -> impl Iterator<Item = &Txn> {
        self.txns.values()
    }

    pub fn txn(&self, id: TxnId) -> Option<&Txn> {
        self.txns.get(&id)
    }

    pub fn attachments(&self) -> impl Iterator<Item = &Attachment> {
        self.attachments.values()
    }

    pub fn attachment(&self, id: AttachmentId) -> Option<&Attachment> {
        self.attachments.get(&id)
    }

    pub fn attachments_of(&self, txn: TxnId) -> impl Iterator<Item = &Attachment> {
        self.attachments.values().filter(move |a| a.txn == txn)
    }

    pub fn closing_counts(&self) -> impl Iterator<Item = &ClosingCount> {
        self.closing_counts.values()
    }

    pub fn closing_count(&self, account: AccountId) -> Option<&ClosingCount> {
        self.closing_counts.get(&account)
    }

    pub fn next_ids(&self) -> NextIds {
        self.next
    }

    /// All derived figures, always up to date.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    pub fn budget_lines(&self) -> impl Iterator<Item = &BudgetLine> {
        self.budget_lines.values()
    }

    pub fn budget_line(&self, id: BudgetLineId) -> Option<&BudgetLine> {
        self.budget_lines.get(&id)
    }

    /// Plan vs actual and the forecast, always up to date.
    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    // --- accounts and persons ------------------------------------------------------------

    /// Adds an asset account (a bank account) or a category. Persons are added with
    /// [`Book::add_person`].
    pub fn add_account(&mut self, kind: AccountKind, name: &str) -> Result<AccountId, DomainError> {
        if !matches!(
            kind,
            AccountKind::Asset | AccountKind::Income | AccountKind::Expense
        ) {
            return Err(DomainError::WrongAccountKind {
                field: "kind".into(),
            });
        }
        self.change(|book| {
            let name = book.unique_name(kind, name, None)?;
            Ok(book.insert_account(kind, &name, None))
        })
    }

    /// Adds a bank or cash account.
    pub fn add_asset(
        &mut self,
        name: &str,
        asset_type: AssetType,
    ) -> Result<AccountId, DomainError> {
        let id = self.add_account(AccountKind::Asset, name)?;
        self.set_asset_type(id, asset_type)?;
        Ok(id)
    }

    /// Makes an asset account a bank or a cash account. Its counts stay.
    pub fn set_asset_type(
        &mut self,
        id: AccountId,
        asset_type: AssetType,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let account = book
                .accounts
                .get_mut(&id)
                .ok_or(DomainError::UnknownAccount { field: "id".into() })?;
            if account.kind != AccountKind::Asset {
                return Err(DomainError::WrongAccountKind { field: "id".into() });
            }
            account.asset_type = Some(asset_type);
            Ok(())
        })
    }

    pub fn rename_account(&mut self, id: AccountId, name: &str) -> Result<(), DomainError> {
        self.change(|book| {
            let kind = book
                .accounts
                .get(&id)
                .ok_or(DomainError::UnknownAccount { field: "id".into() })?
                .kind;
            let name = book.unique_name(kind, name, Some(id))?;
            if let Some(account) = book.accounts.get_mut(&id) {
                account.name = name;
            }
            Ok(())
        })
    }

    pub fn add_person(&mut self, name: &str, person: Person) -> Result<AccountId, DomainError> {
        self.change(|book| {
            let name = book.unique_name(AccountKind::Person, name, None)?;
            let person = clean_person(person)?;
            let id = book.insert_account(AccountKind::Person, &name, None);
            book.persons.insert(id, person);
            Ok(id)
        })
    }

    pub fn update_person(
        &mut self,
        id: AccountId,
        name: &str,
        person: Person,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            if !book.persons.contains_key(&id) {
                return Err(DomainError::UnknownPerson);
            }
            let name = book.unique_name(AccountKind::Person, name, Some(id))?;
            let person = clean_person(person)?;
            if let Some(account) = book.accounts.get_mut(&id) {
                account.name = name;
            }
            book.persons.insert(id, person);
            Ok(())
        })
    }

    // --- transactions --------------------------------------------------------------------

    /// Adds a transaction (`id = None`) or replaces an existing one.
    pub fn save_txn(&mut self, draft: &TxnDraft, id: Option<TxnId>) -> Result<Saved, DomainError> {
        let saved = self.change(|book| {
            check_links(&draft.entry, &book.budget_lines)?;
            let lines = build_lines(&draft.entry, &book.accounts)?;
            if draft.total.is_some_and(|total| total != txn_amount(&lines)) {
                return Err(DomainError::SplitMismatch);
            }
            let id = match id {
                Some(id) if book.txns.contains_key(&id) => id,
                Some(_) => return Err(DomainError::UnknownTxn),
                None => {
                    let id = TxnId(book.next.txn);
                    book.next.txn += 1;
                    id
                }
            };
            book.txns.insert(
                id,
                Txn {
                    id,
                    kind: draft.entry.kind(),
                    date: draft.date,
                    description: draft.description.trim().to_owned(),
                    counterparty: clean(draft.counterparty.clone()),
                    notes: clean(draft.notes.clone()),
                    lines,
                },
            );
            Ok(id)
        })?;

        let mut warnings = Vec::new();
        if let Entry::Settle {
            person, direction, ..
        } = &draft.entry
        {
            let net_after = self
                .ledger
                .person(*person)
                .map(|p| p.net)
                .unwrap_or_default();
            let overshoots = match direction {
                SettleDirection::Reimburse => net_after.is_negative(),
                SettleDirection::Repay => net_after.is_positive(),
            };
            if overshoots {
                warnings.push(Warning::SettleExceedsBalance {
                    person: *person,
                    net_after,
                });
            }
        }
        Ok(Saved {
            id: saved,
            warnings,
        })
    }

    /// Deletes a transaction together with its attachments.
    pub fn delete_txn(&mut self, id: TxnId) -> Result<Txn, DomainError> {
        self.change(|book| book.remove_txn(id).ok_or(DomainError::UnknownTxn))
    }

    // --- attachments and counts ----------------------------------------------------------

    /// Records an attachment's details. The bytes themselves are written by the store.
    pub fn add_attachment(
        &mut self,
        txn: TxnId,
        filename: &str,
        mime: &str,
        size: u64,
        sha256: &str,
        added_at: OffsetDateTime,
    ) -> Result<AttachmentId, DomainError> {
        self.change(|book| {
            if !book.txns.contains_key(&txn) {
                return Err(DomainError::UnknownTxn);
            }
            if size > MAX_ATTACHMENT_SIZE {
                return Err(DomainError::AttachmentTooLarge);
            }
            let filename = filename.trim();
            if filename.is_empty() {
                return Err(DomainError::EmptyName {
                    field: "filename".into(),
                });
            }
            let id = AttachmentId(book.next.attachment);
            book.next.attachment += 1;
            book.attachments.insert(
                id,
                Attachment {
                    id,
                    txn,
                    filename: filename.to_owned(),
                    mime: mime.to_owned(),
                    size,
                    sha256: sha256.to_owned(),
                    added_at,
                },
            );
            Ok(id)
        })
    }

    pub fn remove_attachment(&mut self, id: AttachmentId) -> Result<Attachment, DomainError> {
        self.change(|book| {
            book.attachments
                .remove(&id)
                .ok_or(DomainError::UnknownAttachment)
        })
    }

    /// Records the amount counted in an asset account at the end (R-ISO-3).
    pub fn set_closing_count(
        &mut self,
        account: AccountId,
        date: Date,
        amount: Cents,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            match book.accounts.get(&account) {
                None => {
                    return Err(DomainError::UnknownAccount {
                        field: "account".into(),
                    });
                }
                Some(a) if a.kind != AccountKind::Asset => {
                    return Err(DomainError::WrongAccountKind {
                        field: "account".into(),
                    });
                }
                Some(_) => {}
            }
            if amount.is_negative() {
                return Err(DomainError::NegativeAmount {
                    field: "amount".into(),
                });
            }
            book.closing_counts.insert(
                account,
                ClosingCount {
                    account,
                    date,
                    amount,
                },
            );
            Ok(())
        })
    }

    /// Records the amount counted in an asset account at the start (R-ISO-3) as its one
    /// opening transaction, replacing any it had. Returns that transaction.
    pub fn set_opening_count(
        &mut self,
        account: AccountId,
        date: Date,
        amount: Cents,
    ) -> Result<TxnId, DomainError> {
        self.change(|book| {
            let entry = Entry::Opening { account, amount };
            let lines = build_lines(&entry, &book.accounts)?;
            let existing = book.opening_txns(account);
            let (id, description) = match existing.first() {
                Some(first) => (*first, book.txns[first].description.clone()),
                None => {
                    let id = TxnId(book.next.txn);
                    book.next.txn += 1;
                    (id, OPENING_DESCRIPTION.to_owned())
                }
            };
            for other in existing.iter().skip(1) {
                book.remove_txn(*other);
            }
            let previous = book.txns.get(&id);
            let (counterparty, notes) = previous
                .map(|t| (t.counterparty.clone(), t.notes.clone()))
                .unwrap_or_default();
            book.txns.insert(
                id,
                Txn {
                    id,
                    kind: TxnKind::Opening,
                    date,
                    description,
                    counterparty,
                    notes,
                    lines,
                },
            );
            Ok(id)
        })
    }

    /// Removes the account's opening transactions.
    pub fn clear_opening_count(&mut self, account: AccountId) -> Result<(), DomainError> {
        self.change(|book| {
            for id in book.opening_txns(account) {
                book.remove_txn(id);
            }
            Ok(())
        })
    }

    pub fn clear_closing_count(&mut self, account: AccountId) -> Result<(), DomainError> {
        self.change(|book| {
            book.closing_counts.remove(&account);
            Ok(())
        })
    }

    // --- budget --------------------------------------------------------------------------

    /// Adds a budget line (R-BUD-1).
    pub fn add_budget_line(&mut self, draft: BudgetLineDraft) -> Result<BudgetLineId, DomainError> {
        self.change(|book| {
            let draft = clean_budget_line(draft, &book.accounts)?;
            let id = BudgetLineId(book.next.budget_line);
            book.next.budget_line += 1;
            book.budget_lines.insert(id, BudgetLine { id, draft });
            Ok(id)
        })
    }

    /// Changes a budget line. Its category stays while transaction lines are linked to it,
    /// since a link always stays within its category.
    pub fn update_budget_line(
        &mut self,
        id: BudgetLineId,
        draft: BudgetLineDraft,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let draft = clean_budget_line(draft, &book.accounts)?;
            let line = book
                .budget_lines
                .get(&id)
                .ok_or(DomainError::UnknownBudgetLine)?;
            if line.draft.category != draft.category && book.links_to(id).next().is_some() {
                return Err(DomainError::BudgetLineInUse);
            }
            book.budget_lines.insert(id, BudgetLine { id, draft });
            Ok(())
        })
    }

    /// Deletes a budget line. The transaction lines linked to it stay and count as
    /// unbudgeted from then on. Returns the deleted line.
    pub fn delete_budget_line(&mut self, id: BudgetLineId) -> Result<BudgetLine, DomainError> {
        self.change(|book| {
            let line = book
                .budget_lines
                .remove(&id)
                .ok_or(DomainError::UnknownBudgetLine)?;
            for txn in book.txns.values_mut() {
                for l in &mut txn.lines {
                    if l.budget_line == Some(id) {
                        l.budget_line = None;
                    }
                }
            }
            Ok(line)
        })
    }

    // --- event ---------------------------------------------------------------------------

    pub fn update_event(&mut self, details: EventDetails) -> Result<(), DomainError> {
        self.change(|book| {
            book.event.details = clean_details(details)?;
            Ok(())
        })
    }

    /// Sets where reimbursements come from by default (R-EVT-4). Must be an asset account.
    pub fn set_default_reimbursement_account(&mut self, id: AccountId) -> Result<(), DomainError> {
        self.change(|book| {
            match book.accounts.get(&id) {
                None => {
                    return Err(DomainError::UnknownAccount {
                        field: "account".into(),
                    });
                }
                Some(a) if a.kind != AccountKind::Asset => {
                    return Err(DomainError::WrongAccountKind {
                        field: "account".into(),
                    });
                }
                Some(_) => {}
            }
            book.event.default_reimbursement_account = id;
            Ok(())
        })
    }

    /// Closes the event (R-EVT-2, R-SET-7): from then on it is read-only. Every open item must
    /// be resolved or explicitly overridden with a note. Closing a closed event does nothing.
    pub fn close(
        &mut self,
        overrides: Vec<BlockerOverride>,
        at: OffsetDateTime,
    ) -> Result<(), DomainError> {
        if self.event.is_closed() {
            return Ok(());
        }
        if overrides.iter().any(|o| o.note.trim().is_empty()) {
            return Err(DomainError::MissingNote);
        }
        let open = self.ledger.open_items();
        let blockers: Vec<_> = open
            .iter()
            .filter(|item| !overrides.iter().any(|o| &o.item == *item))
            .cloned()
            .collect();
        if !blockers.is_empty() {
            return Err(DomainError::ClosingBlocked { blockers });
        }
        self.event.closing_overrides = overrides
            .into_iter()
            .filter(|o| open.contains(&o.item))
            .collect();
        self.event.closed_at = Some(at);
        Ok(())
    }

    /// Makes a closed event editable again. The store records it in the change log.
    pub fn reopen(&mut self) {
        self.event.closed_at = None;
        self.event.closing_overrides.clear();
    }

    // --- internals -----------------------------------------------------------------------

    /// Applies `f` to a copy of the book, recomputes the ledger, and keeps the copy only if
    /// both succeed. A closed event rejects every change.
    fn change<T>(
        &mut self,
        f: impl FnOnce(&mut Book) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        if self.event.is_closed() {
            return Err(DomainError::EventClosed);
        }
        let mut candidate = self.clone();
        let value = f(&mut candidate)?;
        candidate.recompute()?;
        *self = candidate;
        Ok(value)
    }

    /// Recomputes the ledger and the budget.
    fn recompute(&mut self) -> Result<(), DomainError> {
        let with_attachments: BTreeSet<TxnId> = self.attachments.values().map(|a| a.txn).collect();
        self.ledger = Ledger::compute(LedgerInput {
            accounts: &self.accounts,
            txns: &self.txns,
            closing_counts: &self.closing_counts,
            with_attachments: &with_attachments,
        })?;
        self.budget =
            Budget::compute(&self.accounts, &self.budget_lines, &self.txns, &self.ledger)?;
        Ok(())
    }

    /// The transaction lines linked to a budget line.
    fn links_to(&self, id: BudgetLineId) -> impl Iterator<Item = &Line> {
        self.txns
            .values()
            .flat_map(|t| &t.lines)
            .filter(move |l| l.budget_line == Some(id))
    }

    fn insert_account(&mut self, kind: AccountKind, name: &str, role: Option<Role>) -> AccountId {
        let id = AccountId(self.next.account);
        self.next.account += 1;
        let asset_type = match (kind, role) {
            (AccountKind::Asset, Some(Role::CashBox)) => Some(AssetType::Cash),
            (AccountKind::Asset, _) => Some(AssetType::Bank),
            _ => None,
        };
        self.accounts.insert(
            id,
            Account {
                id,
                kind,
                name: name.to_owned(),
                role,
                asset_type,
            },
        );
        id
    }

    fn remove_txn(&mut self, id: TxnId) -> Option<Txn> {
        let txn = self.txns.remove(&id)?;
        self.attachments.retain(|_, a| a.txn != id);
        Some(txn)
    }

    /// The opening transactions that bring money into `account`, oldest first.
    fn opening_txns(&self, account: AccountId) -> Vec<TxnId> {
        self.txns
            .values()
            .filter(|t| t.kind == TxnKind::Opening && t.lines.iter().any(|l| l.account == account))
            .map(|t| t.id)
            .collect()
    }

    /// A trimmed, non-empty name that no other account of the same kind uses (ignoring case).
    fn unique_name(
        &self,
        kind: AccountKind,
        name: &str,
        except: Option<AccountId>,
    ) -> Result<String, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::EmptyName {
                field: "name".into(),
            });
        }
        match self.account_by_name(kind, name) {
            Some(other) if Some(other.id) != except => Err(DomainError::DuplicateName {
                field: "name".into(),
            }),
            _ => Ok(name.to_owned()),
        }
    }
}

fn clean(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty())
}

fn clean_details(d: EventDetails) -> Result<EventDetails, DomainError> {
    let name = d.name.trim().to_owned();
    if name.is_empty() {
        return Err(DomainError::EmptyName {
            field: "name".into(),
        });
    }
    Ok(EventDetails {
        name,
        location: clean(d.location),
        notes: clean(d.notes),
        ..d
    })
}

/// A trimmed description, an income or expense category and an estimate that fits.
fn clean_budget_line(
    d: BudgetLineDraft,
    accounts: &BTreeMap<AccountId, Account>,
) -> Result<BudgetLineDraft, DomainError> {
    match accounts.get(&d.category).map(|a| a.kind) {
        None => {
            return Err(DomainError::UnknownAccount {
                field: "category".into(),
            });
        }
        Some(AccountKind::Income | AccountKind::Expense) => {}
        Some(_) => {
            return Err(DomainError::WrongAccountKind {
                field: "category".into(),
            });
        }
    }
    let description = d.description.trim().to_owned();
    if description.is_empty() {
        return Err(DomainError::EmptyName {
            field: "description".into(),
        });
    }
    match d.estimate {
        Estimate::Amount { amount } if amount.is_negative() => {
            return Err(DomainError::NegativeAmount {
                field: "estimate.amount".into(),
            });
        }
        Estimate::Quantity { quantity: 0, .. } => {
            return Err(DomainError::NonPositiveAmount {
                field: "estimate.quantity".into(),
            });
        }
        Estimate::Quantity { unit_price, .. } if unit_price.is_negative() => {
            return Err(DomainError::NegativeAmount {
                field: "estimate.unitPrice".into(),
            });
        }
        _ => {}
    }
    d.estimate.total()?;
    Ok(BudgetLineDraft { description, ..d })
}

/// Every link must point to an existing budget line of the line's own category (R-BUD-2).
/// Errors name the form's split line.
fn check_links(
    entry: &Entry,
    budget_lines: &BTreeMap<BudgetLineId, BudgetLine>,
) -> Result<(), DomainError> {
    let (Entry::Expense { lines, .. } | Entry::Income { lines, .. }) = entry else {
        return Ok(());
    };
    for (i, l) in lines.iter().enumerate() {
        let Some(id) = l.budget_line else { continue };
        let b = budget_lines
            .get(&id)
            .ok_or(DomainError::UnknownBudgetLine)?;
        if b.draft.category != l.account {
            return Err(DomainError::WrongBudgetLine {
                field: format!("lines[{i}].budgetLine"),
            });
        }
    }
    Ok(())
}

/// The stored form of [`check_links`], for loading a file.
fn links_valid(lines: &[Line], budget_lines: &BTreeMap<BudgetLineId, BudgetLine>) -> bool {
    lines.iter().all(|l| {
        l.budget_line.is_none_or(|id| {
            budget_lines
                .get(&id)
                .is_some_and(|b| b.draft.category == l.account)
        })
    })
}

fn clean_person(p: Person) -> Result<Person, DomainError> {
    let bic = match clean(p.bic) {
        Some(bic) => Some(validate_bic(&bic).map_err(|_| DomainError::InvalidBic)?),
        None => None,
    };
    Ok(Person {
        holder: clean(p.holder),
        iban: p.iban,
        bic,
        contact: clean(p.contact),
        notes: clean(p.notes),
    })
}
