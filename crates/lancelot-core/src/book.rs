//! The event in memory: its details, accounts, persons and transactions, plus the derived
//! [`Ledger`]. Every change is applied to a copy, checked, and only then kept, so a `Book`
//! is always consistent. [`crate::store::Store`] loads a `Book` and writes each accepted change.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use time::{Date, OffsetDateTime};
use ts_rs::TS;

use crate::budget::Budget;
use crate::entry::{Entry, SettleDirection, TxnDraft, build_lines, txn_amount, txn_follows_rules};
use crate::error::DomainError;
use crate::iban::validate_bic;
use crate::ledger::{Ledger, LedgerInput, TicketIncome};
use crate::model::{
    Account, AccountId, AccountKind, AssetType, Attachment, AttachmentId, BlockerOverride,
    BudgetLine, BudgetLineDraft, BudgetLineId, ClosingCount, CostCentre, CostCentreId, Estimate,
    Event, EventDetails, Line, OpenItem, Person, PluginRecordId, Reconciliation, ReconciliationId,
    Role, Txn, TxnId, TxnKind,
};
use crate::model::{CancellationId, ImportId, ManualSaleId, Origin, TierId};
use crate::money::Cents;
use crate::payout::{Payouts, payouts};
use crate::plugin::PluginRecord;
use crate::sales::Profitability;
use crate::tickets::{Cancellation, ManualSale, TicketImport, TicketSettings, Tickets, Tier};

mod cost_centres;
mod plugins;
mod template;
mod tickets;

pub use template::{CopyChoice, CopySummary, TemplateSummary};

/// The largest attachment Lancelot accepts. Phone photos and PDFs are far smaller; the limit
/// only catches accidents.
pub const MAX_ATTACHMENT_SIZE: u64 = 50 * 1024 * 1024;

/// Default categories of a new event (R-ACC-2).
const DEFAULT_INCOME: &[&str] = &["Tickets", "General"];
const DEFAULT_EXPENSE: &[&str] = &["Kitchen", "Mobility", "Venue", "Tech", "General"];

/// The description of an opening transaction made by [`Book::set_opening_count`].
const OPENING_DESCRIPTION: &str = "Opening count";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    event: Event,
    ticket_settings: TicketSettings,
    accounts: BTreeMap<AccountId, Account>,
    persons: BTreeMap<AccountId, Person>,
    txns: BTreeMap<TxnId, Txn>,
    attachments: BTreeMap<AttachmentId, Attachment>,
    closing_counts: BTreeMap<AccountId, ClosingCount>,
    reconciliations: BTreeMap<ReconciliationId, Reconciliation>,
    budget_lines: BTreeMap<BudgetLineId, BudgetLine>,
    tiers: BTreeMap<TierId, Tier>,
    imports: BTreeMap<ImportId, TicketImport>,
    manual_sales: BTreeMap<ManualSaleId, ManualSale>,
    cancellations: BTreeMap<CancellationId, Cancellation>,
    cost_centres: BTreeMap<CostCentreId, CostCentre>,
    plugin_records: BTreeMap<PluginRecordId, PluginRecord>,
    next: NextIds,
    ledger: Ledger,
    budget: Budget,
    tickets: Tickets,
    payouts: Payouts,
    /// Regardless of whether the event is closed: see [`Book::profitability`].
    profitability: Option<Profitability>,
}

/// The ids the next new account, transaction, attachment, budget line, tier, import, manual
/// sale, cancellation, cost centre, plugin record and checkpoint get. Ids are never reused, so the change log stays unambiguous after deletions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextIds {
    pub account: i64,
    pub txn: i64,
    pub attachment: i64,
    pub budget_line: i64,
    pub tier: i64,
    pub import: i64,
    pub manual_sale: i64,
    pub cancellation: i64,
    pub cost_centre: i64,
    pub plugin_record: i64,
    pub reconciliation: i64,
}

/// A book taken apart into its stored pieces. [`Book::from_parts`] checks them all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookParts {
    pub event: Event,
    pub ticket_settings: TicketSettings,
    pub accounts: Vec<Account>,
    pub persons: Vec<(AccountId, Person)>,
    pub txns: Vec<Txn>,
    pub attachments: Vec<Attachment>,
    pub closing_counts: Vec<ClosingCount>,
    pub reconciliations: Vec<Reconciliation>,
    pub budget_lines: Vec<BudgetLine>,
    pub tiers: Vec<Tier>,
    pub imports: Vec<TicketImport>,
    pub manual_sales: Vec<ManualSale>,
    pub cancellations: Vec<Cancellation>,
    pub cost_centres: Vec<CostCentre>,
    pub plugin_records: Vec<PluginRecord>,
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
            ticket_settings: TicketSettings {
                enabled: true,
                account: None,
            },
            accounts: BTreeMap::new(),
            persons: BTreeMap::new(),
            txns: BTreeMap::new(),
            attachments: BTreeMap::new(),
            closing_counts: BTreeMap::new(),
            reconciliations: BTreeMap::new(),
            budget_lines: BTreeMap::new(),
            tiers: BTreeMap::new(),
            imports: BTreeMap::new(),
            manual_sales: BTreeMap::new(),
            cancellations: BTreeMap::new(),
            cost_centres: BTreeMap::new(),
            plugin_records: BTreeMap::new(),
            next: NextIds {
                account: 1,
                txn: 1,
                attachment: 1,
                budget_line: 1,
                tier: 1,
                import: 1,
                manual_sale: 1,
                cancellation: 1,
                cost_centre: 1,
                plugin_record: 1,
                reconciliation: 1,
            },
            ledger: Ledger::default(),
            budget: Budget::default(),
            tickets: Tickets::default(),
            payouts: Payouts::default(),
            profitability: None,
        };
        let verein = book.insert_account(AccountKind::Asset, "Verein account", None);
        let cash = book.insert_account(AccountKind::Asset, "Cash box", None);
        book.insert_account(AccountKind::Opening, "Opening balance", None);
        for name in DEFAULT_INCOME {
            let role = (*name == "Tickets").then_some(Role::Tickets);
            book.insert_account(AccountKind::Income, name, role);
        }
        for name in DEFAULT_EXPENSE {
            book.insert_account(AccountKind::Expense, name, None);
        }
        if let Some(a) = book.accounts.get_mut(&verein) {
            a.net_movement = true;
        }
        if let Some(a) = book.accounts.get_mut(&cash) {
            a.asset_type = Some(AssetType::Cash);
        }
        book.ticket_settings.account = Some(verein);
        book.event.default_reimbursement_account = verein;
        book.recompute()?;
        Ok(book)
    }

    /// Rebuilds a book from stored pieces, checking everything a hand-edited or damaged
    /// file could get wrong: settings, kinds, names, transaction rules, references and ids.
    pub fn from_parts(parts: BookParts) -> Result<Book, DomainError> {
        Ok(Book::load(parts)?.1)
    }

    /// Checks everything in the parts and builds the book as stored, without its derived figures.
    fn stored(parts: BookParts) -> Result<Book, DomainError> {
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
        let openings = accounts
            .values()
            .filter(|a| a.kind == AccountKind::Opening)
            .count();
        if openings != 1 {
            return Err(bad("the opening account must exist exactly once"));
        }
        let roles_ok = accounts.values().all(|a| match a.role {
            Some(Role::Tickets | Role::TicketRefunds) => a.kind == AccountKind::Income,
            Some(Role::TicketFees) => a.kind == AccountKind::Expense,
            None => true,
        });
        let once = |role| accounts.values().filter(|a| a.role == Some(role)).count() <= 1;
        let types_ok = accounts.values().all(|a| {
            a.asset_type.is_some() == (a.kind == AccountKind::Asset)
                && (a.kind == AccountKind::Asset || !a.net_movement)
        });
        let names: BTreeSet<(AccountKind, String)> = accounts
            .values()
            .map(|a| (a.kind, a.name.to_ascii_lowercase()))
            .collect();
        if !roles_ok
            || !types_ok
            || !once(Role::Tickets)
            || !once(Role::TicketFees)
            || !once(Role::TicketRefunds)
            || names.len() != accounts.len()
        {
            return Err(bad("account kinds, roles or names are inconsistent"));
        }

        let cost_centres =
            cost_centres::check_cost_centres(parts.cost_centres, &parts.next, &accounts)?;

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
                || clean_budget_line(b.draft.clone(), &accounts, &cost_centres).as_ref()
                    != Ok(&b.draft)
            {
                return Err(bad("budget line with an invalid category or estimate"));
            }
            budget_lines.insert(b.id, b);
        }

        let mut txns = BTreeMap::new();
        for t in parts.txns {
            if t.id.0 >= parts.next.txn
                || !txn_follows_rules(&t, &accounts)
                || !links_valid(&t.lines, &budget_lines)
                || !tags_valid(&t.lines, &accounts, &cost_centres)
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
            let account = accounts.get(&c.account);
            if account.map(|a| a.kind) != Some(AccountKind::Asset)
                || (c.amount.is_negative()
                    && account.and_then(|a| a.asset_type) != Some(AssetType::Bank))
            {
                return Err(bad(
                    "closing count on an account that is not an asset, or a negative count of cash",
                ));
            }
            closing_counts.insert(c.account, c);
        }

        let mut reconciliations = BTreeMap::new();
        let mut checkpoints = BTreeSet::new();
        for r in parts.reconciliations {
            if accounts.get(&r.account).map(|a| a.kind) != Some(AccountKind::Asset)
                || r.id.0 >= parts.next.reconciliation
                || !checkpoints.insert((r.account, r.date))
            {
                return Err(bad(
                    "checkpoint on an account that is not an asset, or twice",
                ));
            }
            reconciliations.insert(r.id, r);
        }

        let default = parts.event.default_reimbursement_account;
        if accounts.get(&default).map(|a| a.kind) != Some(AccountKind::Asset) {
            return Err(bad("default reimbursement account is not an asset"));
        }

        let plugin_records = plugins::check_records(parts.plugin_records, &parts.next)?;

        let (tiers, imports, manual_sales, cancellations) = tickets::check_parts(
            parts.tiers,
            parts.imports,
            parts.manual_sales,
            parts.cancellations,
            &parts.next,
            &accounts,
        )?;
        let ticket_settings = parts.ticket_settings;
        let account_ok = match ticket_settings.account {
            Some(id) => accounts.get(&id).map(|a| a.kind) == Some(AccountKind::Asset),
            None => imports.is_empty(),
        };
        if !account_ok || (!ticket_settings.enabled && !tiers.is_empty()) {
            return Err(bad("ticket settings do not fit the tickets"));
        }

        let book = Book {
            event: parts.event,
            ticket_settings,
            accounts,
            persons,
            txns,
            attachments,
            closing_counts,
            reconciliations,
            budget_lines,
            tiers,
            imports,
            manual_sales,
            cancellations,
            cost_centres,
            plugin_records,
            next: parts.next,
            ledger: Ledger::default(),
            budget: Budget::default(),
            tickets: Tickets::default(),
            payouts: Payouts::default(),
            profitability: None,
        };
        Ok(book)
    }

    /// The book as it is stored, and as it is once the ticket income is regenerated. The tickets
    /// are the source of truth; the stored entries only keep the file readable without Lancelot.
    pub(crate) fn load(parts: BookParts) -> Result<(Book, Book), DomainError> {
        let stored = Book::stored(parts)?;
        let mut book = stored.clone();
        book.sync_tickets()?;
        book.recompute()?;
        Ok((stored, book))
    }

    /// The generated entries that differ between two loads of a file, and how many of them
    /// differ in their money, not only in wording.
    pub(crate) fn regenerated(&self, stored: &Book) -> (bool, usize) {
        let money = |book: &Book| -> BTreeMap<Origin, Vec<(AccountId, Cents)>> {
            book.txns
                .values()
                .filter(|t| t.origin.is_generated())
                .map(|t| {
                    (
                        t.origin,
                        t.lines.iter().map(|l| (l.account, l.amount)).collect(),
                    )
                })
                .collect()
        };
        let (now, then) = (money(self), money(stored));
        let moved = now
            .iter()
            .filter(|(origin, lines)| then.get(origin) != Some(lines))
            .count()
            + then
                .keys()
                .filter(|origin| !now.contains_key(origin))
                .count();
        (
            self.txns != stored.txns || self.accounts != stored.accounts,
            moved,
        )
    }

    // --- reading -------------------------------------------------------------------------

    /// What keeps the event from being closed: the ledger's items, then the tickets' (R-SET-5).
    pub fn open_items(&self) -> Vec<OpenItem> {
        let mut items = self.ledger.open_items();
        items.extend(self.tickets.open_item());
        items
    }

    pub fn event(&self) -> &Event {
        &self.event
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    pub(crate) fn account_map(&self) -> &BTreeMap<AccountId, Account> {
        &self.accounts
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

    pub fn reconciliations(&self) -> impl Iterator<Item = &Reconciliation> {
        self.reconciliations.values()
    }

    pub fn next_ids(&self) -> NextIds {
        self.next
    }

    /// All derived figures, always up to date.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// The payout and repayment lists (R-PER-4, R-PER-7).
    pub fn payouts(&self) -> &Payouts {
        &self.payouts
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
            let negative = book
                .closing_counts
                .get(&id)
                .is_some_and(|c| c.amount.is_negative());
            if asset_type == AssetType::Cash && negative {
                return Err(DomainError::NegativeCashCount);
            }
            Ok(())
        })
    }

    /// Deletes an account or a person nothing refers to, such as a mistyped one (not the opening
    /// account or a ticket category). Whatever is in use stays, so no figure changes.
    pub fn delete_account(&mut self, id: AccountId) -> Result<Account, DomainError> {
        self.change(|book| {
            let account = book
                .accounts
                .get(&id)
                .ok_or(DomainError::UnknownAccount { field: "id".into() })?;
            let in_use = account.role.is_some()
                || account.kind == AccountKind::Opening
                || book.event.default_reimbursement_account == id
                || book.ticket_settings.account == Some(id)
                || book.closing_counts.contains_key(&id)
                || book.reconciliations.values().any(|r| r.account == id)
                || book.category_in_use(id)
                || book
                    .manual_sales
                    .values()
                    .any(|s| s.draft.destination == id)
                || book
                    .cancellations
                    .values()
                    .any(|c| c.terms.is_some_and(|t| t.payer == Some(id)));
            if in_use {
                return Err(DomainError::AccountInUse);
            }
            book.persons.remove(&id);
            book.accounts
                .remove(&id)
                .ok_or(DomainError::UnknownAccount { field: "id".into() })
        })
    }

    /// Marks an asset account as holding only the event's net movement (R-ISO-2). Its counts and
    /// checkpoints are compared without opening counts from then on.
    pub fn set_net_movement(
        &mut self,
        id: AccountId,
        net_movement: bool,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let account = book
                .accounts
                .get_mut(&id)
                .ok_or(DomainError::UnknownAccount { field: "id".into() })?;
            if account.kind != AccountKind::Asset {
                return Err(DomainError::WrongAccountKind { field: "id".into() });
            }
            account.net_movement = net_movement;
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
            check_links(&draft.entry, &book.budget_lines, &book.cost_centres)?;
            let lines = build_lines(&draft.entry, &book.accounts)?;
            if draft.total.is_some_and(|total| total != txn_amount(&lines)) {
                return Err(DomainError::SplitMismatch);
            }
            let id = match id.map(|id| (id, book.txns.get(&id))) {
                Some((_, Some(t))) if t.origin.is_generated() => {
                    return Err(DomainError::GeneratedTxn);
                }
                Some((id, Some(_))) => id,
                Some((_, None)) => return Err(DomainError::UnknownTxn),
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
                    origin: Origin::Manual,
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

    /// Deletes a transaction together with its attachments. Generated ones go away with the
    /// tickets they come from.
    pub fn delete_txn(&mut self, id: TxnId) -> Result<Txn, DomainError> {
        self.change(|book| {
            if book.txns.get(&id).is_some_and(|t| t.origin.is_generated()) {
                return Err(DomainError::GeneratedTxn);
            }
            book.remove_txn(id).ok_or(DomainError::UnknownTxn)
        })
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
            let asset_type = book.accounts.get(&account).and_then(|a| a.asset_type);
            if amount.is_negative() && asset_type != Some(AssetType::Bank) {
                return Err(DomainError::NegativeCashCount);
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
                    origin: Origin::Manual,
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

    // --- reconciliation ------------------------------------------------------------------

    /// Records what a statement shows for an asset account on a date (R-ACC-4). A mismatch
    /// becomes an open item.
    pub fn add_reconciliation(
        &mut self,
        account: AccountId,
        date: Date,
        amount: Cents,
    ) -> Result<ReconciliationId, DomainError> {
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
            book.check_checkpoint_free(account, date, None)?;
            let id = ReconciliationId(book.next.reconciliation);
            book.next.reconciliation += 1;
            book.reconciliations.insert(
                id,
                Reconciliation {
                    id,
                    account,
                    date,
                    amount,
                },
            );
            Ok(id)
        })
    }

    /// Corrects a checkpoint's date or amount. Its account stays.
    pub fn update_reconciliation(
        &mut self,
        id: ReconciliationId,
        date: Date,
        amount: Cents,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let account = book
                .reconciliations
                .get(&id)
                .ok_or(DomainError::UnknownReconciliation)?
                .account;
            book.check_checkpoint_free(account, date, Some(id))?;
            book.reconciliations.insert(
                id,
                Reconciliation {
                    id,
                    account,
                    date,
                    amount,
                },
            );
            Ok(())
        })
    }

    pub fn delete_reconciliation(
        &mut self,
        id: ReconciliationId,
    ) -> Result<Reconciliation, DomainError> {
        self.change(|book| {
            book.reconciliations
                .remove(&id)
                .ok_or(DomainError::UnknownReconciliation)
        })
    }

    // --- budget --------------------------------------------------------------------------

    /// Adds a budget line (R-BUD-1).
    pub fn add_budget_line(&mut self, draft: BudgetLineDraft) -> Result<BudgetLineId, DomainError> {
        self.change(|book| {
            let draft = clean_budget_line(draft, &book.accounts, &book.cost_centres)?;
            let id = BudgetLineId(book.next.budget_line);
            book.next.budget_line += 1;
            book.budget_lines.insert(id, BudgetLine { id, draft });
            Ok(id)
        })
    }

    /// Changes a budget line. Its category stays while transaction lines are linked to it,
    /// since a link always stays within its category. Linked lines follow its cost centre.
    pub fn update_budget_line(
        &mut self,
        id: BudgetLineId,
        draft: BudgetLineDraft,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let draft = clean_budget_line(draft, &book.accounts, &book.cost_centres)?;
            let line = book
                .budget_lines
                .get(&id)
                .ok_or(DomainError::UnknownBudgetLine)?;
            if line.draft.category != draft.category && book.links_to(id).next().is_some() {
                return Err(DomainError::BudgetLineInUse);
            }
            for l in book.txns.values_mut().flat_map(|t| &mut t.lines) {
                if l.budget_line == Some(id) {
                    l.cost_centre = draft.cost_centre;
                }
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
        let open = self.open_items();
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

    /// Makes a closed event editable again. The store records it in the change log. The closing
    /// notes stay, to fill in the items that still match when it is closed again (R-SET-7).
    pub fn reopen(&mut self) -> Result<(), DomainError> {
        self.event.closed_at = None;
        Ok(())
    }

    /// Records every reimbursement on the payout list that has an IBAN, from `account` on
    /// `date` (R-PER-5). Returns how many were recorded.
    pub fn mark_payouts_paid(
        &mut self,
        account: AccountId,
        date: Date,
    ) -> Result<usize, DomainError> {
        let list = self.payouts.clone();
        self.change(|b| {
            let mut n = 0;
            for p in list.payouts.iter().filter(|p| p.iban.is_some()) {
                let draft = TxnDraft {
                    date,
                    description: "Reimbursement".into(),
                    counterparty: None,
                    notes: Some(p.reference.clone()),
                    entry: Entry::Settle {
                        person: p.person,
                        account,
                        amount: p.amount,
                        direction: SettleDirection::Reimburse,
                    },
                    total: None,
                };
                b.save_txn(&draft, None)?;
                n += 1;
            }
            Ok(n)
        })
    }

    /// Replaces both counts of an asset account in one change, as the count panel saves them.
    /// `None` removes a count, and so does an opening count of zero.
    pub fn set_counts(
        &mut self,
        account: AccountId,
        opening: Option<(Date, Cents)>,
        closing: Option<(Date, Cents)>,
    ) -> Result<(), DomainError> {
        self.change(|b| {
            if b.account(account).map(|a| a.kind) != Some(AccountKind::Asset) {
                return Err(DomainError::WrongAccountKind {
                    field: "account".into(),
                });
            }
            match opening {
                Some((_, amount)) if amount.is_negative() => {
                    return Err(DomainError::NegativeAmount {
                        field: "opening".into(),
                    });
                }
                Some((date, amount)) if amount.is_positive() => {
                    b.set_opening_count(account, date, amount)?;
                }
                _ => b.clear_opening_count(account)?,
            }
            match closing {
                Some((date, amount)) => b.set_closing_count(account, date, amount),
                None => b.clear_closing_count(account),
            }
        })
    }

    // --- internals -----------------------------------------------------------------------

    /// Applies `f` to a copy of the book, regenerates the ticket income, recomputes the ledger,
    /// and keeps the copy only if all succeed. A closed event rejects every change.
    fn change<T>(
        &mut self,
        f: impl FnOnce(&mut Book) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        if self.event.is_closed() {
            return Err(DomainError::EventClosed);
        }
        let mut candidate = self.clone();
        let value = f(&mut candidate)?;
        candidate.sync_tickets()?;
        candidate.recompute()?;
        *self = candidate;
        Ok(value)
    }

    /// Recomputes the ticket figures, the ledger and the budget.
    fn recompute(&mut self) -> Result<(), DomainError> {
        self.tickets = Tickets::compute(
            &self.tiers,
            &self.imports,
            &self.manual_sales,
            &self.cancellations,
            &self.generated_txns(),
            self.ticket_settings.account,
        )?;
        let with_attachments: BTreeSet<TxnId> = self.attachments.values().map(|a| a.txn).collect();
        self.ledger = Ledger::compute(LedgerInput {
            accounts: &self.accounts,
            txns: &self.txns,
            closing_counts: &self.closing_counts,
            reconciliations: &self.reconciliations,
            with_attachments: &with_attachments,
            tickets: TicketIncome {
                account: self.ticket_settings.account,
                sheet_income: &crate::tickets::sheet_income(
                    &self.tiers,
                    &self.imports,
                    &self.cancellations,
                )?,
            },
        })?;
        self.budget = Budget::compute(
            &self.accounts,
            &self.budget_lines,
            &self.cost_centres,
            &self.txns,
            &self.ledger,
        )?;
        self.payouts = payouts(self)?;
        self.profitability = if self.budget_lines.is_empty() || !self.ticket_settings.enabled {
            None
        } else {
            Some(Profitability::compute(&self.tickets, &self.budget)?)
        };
        Ok(())
    }

    /// Whether a budget line or a transaction line books to the account.
    fn category_in_use(&self, id: AccountId) -> bool {
        self.budget_lines.values().any(|b| b.draft.category == id)
            || self
                .txns
                .values()
                .flat_map(|t| &t.lines)
                .any(|l| l.account == id)
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
        let asset_type = (kind == AccountKind::Asset).then_some(AssetType::Bank);
        self.accounts.insert(
            id,
            Account {
                id,
                kind,
                name: name.to_owned(),
                role,
                asset_type,
                net_movement: false,
                cost_centre: None,
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

    /// Two checkpoints on one date would be compared with the same figure.
    fn check_checkpoint_free(
        &self,
        account: AccountId,
        date: Date,
        except: Option<ReconciliationId>,
    ) -> Result<(), DomainError> {
        let taken = self
            .reconciliations
            .values()
            .any(|r| r.account == account && r.date == date && Some(r.id) != except);
        if taken {
            return Err(DomainError::DuplicateCheckpoint {
                field: "date".into(),
            });
        }
        Ok(())
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

fn a_category(kind: AccountKind) -> bool {
    matches!(kind, AccountKind::Income | AccountKind::Expense)
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
    cost_centres: &BTreeMap<CostCentreId, CostCentre>,
) -> Result<BudgetLineDraft, DomainError> {
    if d.cost_centre
        .is_some_and(|c| !cost_centres.contains_key(&c))
    {
        return Err(DomainError::UnknownCostCentre {
            field: "costCentre".into(),
        });
    }
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

/// Every link must point to an existing budget line of the line's own category (R-BUD-2), and
/// the line has that budget line's cost centre (R-CC-1). Errors name the form's split line.
fn check_links(
    entry: &Entry,
    budget_lines: &BTreeMap<BudgetLineId, BudgetLine>,
    cost_centres: &BTreeMap<CostCentreId, CostCentre>,
) -> Result<(), DomainError> {
    let (Entry::Expense { lines, .. } | Entry::Income { lines, .. }) = entry else {
        return Ok(());
    };
    for (i, l) in lines.iter().enumerate() {
        if l.cost_centre
            .is_some_and(|c| !cost_centres.contains_key(&c))
        {
            return Err(DomainError::UnknownCostCentre {
                field: format!("lines[{i}].costCentre"),
            });
        }
        let Some(id) = l.budget_line else { continue };
        let b = budget_lines
            .get(&id)
            .ok_or(DomainError::UnknownBudgetLine)?;
        if b.draft.category != l.account {
            return Err(DomainError::WrongBudgetLine {
                field: format!("lines[{i}].budgetLine"),
            });
        }
        if b.draft.cost_centre != l.cost_centre {
            return Err(DomainError::WrongCostCentre {
                field: format!("lines[{i}].costCentre"),
            });
        }
    }
    Ok(())
}

/// The stored form of [`check_links`], for loading a file.
fn links_valid(lines: &[Line], budget_lines: &BTreeMap<BudgetLineId, BudgetLine>) -> bool {
    lines.iter().all(|l| {
        l.budget_line.is_none_or(|id| {
            budget_lines.get(&id).is_some_and(|b| {
                b.draft.category == l.account && b.draft.cost_centre == l.cost_centre
            })
        })
    })
}

/// Only category lines are tagged, and only with a cost centre that exists (R-CC-1).
fn tags_valid(
    lines: &[Line],
    accounts: &BTreeMap<AccountId, Account>,
    cost_centres: &BTreeMap<CostCentreId, CostCentre>,
) -> bool {
    lines.iter().all(|l| {
        l.cost_centre.is_none_or(|c| {
            cost_centres.contains_key(&c)
                && accounts.get(&l.account).is_some_and(|a| a_category(a.kind))
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
