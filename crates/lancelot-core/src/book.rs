//! The event in memory: its details, accounts, persons and transactions, plus the derived
//! [`Ledger`]. Every change is applied to a copy, checked, and only then kept, so a `Book`
//! is always consistent. Storage (M1c) loads a `Book` and writes each accepted change.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::entry::{Entry, SettleDirection, TxnDraft, build_lines};
use crate::error::DomainError;
use crate::iban::validate_bic;
use crate::ledger::{BlockerOverride, Ledger};
use crate::model::{
    Account, AccountId, AccountKind, Event, EventDetails, EventState, Person, Role, Txn, TxnId,
};
use crate::money::Cents;

/// Default categories of a new event (R-ACC-2).
const DEFAULT_INCOME: &[&str] = &["Tickets", "Drink donations", "Other"];
const DEFAULT_EXPENSE: &[&str] = &["Food", "Venue", "Drinks", "Pfand", "Tech", "Other"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    event: Event,
    accounts: BTreeMap<AccountId, Account>,
    persons: BTreeMap<AccountId, Person>,
    txns: BTreeMap<TxnId, Txn>,
    next_account: i64,
    next_txn: i64,
    ledger: Ledger,
}

/// The outcome of saving a transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Saved {
    pub id: TxnId,
    pub warnings: Vec<Warning>,
}

/// Accepted, but worth pointing out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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
                state: EventState::Planning,
                default_reimbursement_account: AccountId(1),
                closing_overrides: Vec::new(),
            },
            accounts: BTreeMap::new(),
            persons: BTreeMap::new(),
            txns: BTreeMap::new(),
            next_account: 1,
            next_txn: 1,
            ledger: Ledger::default(),
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
        book.ledger = Ledger::compute(&book.accounts, book.txns.values())?;
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

    /// All derived figures, always up to date.
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    // --- accounts and persons ------------------------------------------------------------

    /// Adds an asset account or a category. Persons are added with [`Book::add_person`].
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
            let lines = build_lines(&draft.entry, &book.accounts)?;
            let id = match id {
                Some(id) if book.txns.contains_key(&id) => id,
                Some(_) => return Err(DomainError::UnknownTxn),
                None => {
                    let id = TxnId(book.next_txn);
                    book.next_txn += 1;
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

    pub fn delete_txn(&mut self, id: TxnId) -> Result<Txn, DomainError> {
        self.change(|book| book.txns.remove(&id).ok_or(DomainError::UnknownTxn))
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

    /// Moves the event to another state (R-EVT-2).
    ///
    /// Any move is allowed, except: closing needs every open item resolved or overridden
    /// with a note (R-SET-7), and a closed event can only be reopened to Settling.
    pub fn set_state(
        &mut self,
        to: EventState,
        overrides: Vec<BlockerOverride>,
    ) -> Result<(), DomainError> {
        let from = self.event.state;
        if from == to {
            return Ok(());
        }
        if from == EventState::Closed && to != EventState::Settling {
            return Err(DomainError::InvalidStateChange);
        }
        if to == EventState::Closed {
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
        } else {
            self.event.closing_overrides.clear();
        }
        self.event.state = to;
        Ok(())
    }

    // --- internals -----------------------------------------------------------------------

    /// Applies `f` to a copy of the book, recomputes the ledger, and keeps the copy only if
    /// both succeed. A closed event rejects every change.
    fn change<T>(
        &mut self,
        f: impl FnOnce(&mut Book) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        if self.event.state == EventState::Closed {
            return Err(DomainError::EventClosed);
        }
        let mut candidate = self.clone();
        let value = f(&mut candidate)?;
        candidate.ledger = Ledger::compute(&candidate.accounts, candidate.txns.values())?;
        *self = candidate;
        Ok(value)
    }

    fn insert_account(&mut self, kind: AccountKind, name: &str, role: Option<Role>) -> AccountId {
        let id = AccountId(self.next_account);
        self.next_account += 1;
        self.accounts.insert(
            id,
            Account {
                id,
                kind,
                name: name.to_owned(),
                role,
            },
        );
        id
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
