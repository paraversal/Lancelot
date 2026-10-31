//! The book's cost centres (requirements §5.8).

use std::collections::{BTreeMap, BTreeSet};

use super::{Book, NextIds, a_category};
use crate::error::DomainError;
use crate::model::{Account, AccountId, CostCentre, CostCentreId};

impl Book {
    pub fn cost_centres(&self) -> impl Iterator<Item = &CostCentre> {
        self.cost_centres.values()
    }

    pub fn cost_centre(&self, id: CostCentreId) -> Option<&CostCentre> {
        self.cost_centres.get(&id)
    }

    pub fn add_cost_centre(&mut self, name: &str) -> Result<CostCentreId, DomainError> {
        self.change(|book| {
            let name = book.unique_centre_name(name, None)?;
            Ok(book.insert_cost_centre(&name))
        })
    }

    pub fn rename_cost_centre(&mut self, id: CostCentreId, name: &str) -> Result<(), DomainError> {
        self.change(|book| {
            let name = book.unique_centre_name(name, Some(id))?;
            let centre = book.cost_centres.get_mut(&id).ok_or_else(unknown_centre)?;
            centre.name = name;
            Ok(())
        })
    }

    /// Deletes a cost centre. What was tagged with it stays, without a tag.
    pub fn delete_cost_centre(&mut self, id: CostCentreId) -> Result<CostCentre, DomainError> {
        self.change(|book| {
            for l in book.txns.values_mut().flat_map(|t| &mut t.lines) {
                if l.cost_centre == Some(id) {
                    l.cost_centre = None;
                }
            }
            for b in book.budget_lines.values_mut() {
                if b.draft.cost_centre == Some(id) {
                    b.draft.cost_centre = None;
                }
            }
            for a in book.accounts.values_mut() {
                if a.cost_centre == Some(id) {
                    a.cost_centre = None;
                }
            }
            book.cost_centres.remove(&id).ok_or_else(unknown_centre)
        })
    }

    /// Sets the cost centre a new line of this category starts with. Existing lines keep theirs.
    pub fn set_category_cost_centre(
        &mut self,
        category: AccountId,
        cost_centre: Option<CostCentreId>,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            if cost_centre.is_some_and(|c| !book.cost_centres.contains_key(&c)) {
                return Err(unknown_centre());
            }
            let account = book
                .accounts
                .get_mut(&category)
                .ok_or(DomainError::UnknownAccount {
                    field: "category".into(),
                })?;
            if !a_category(account.kind) {
                return Err(DomainError::WrongAccountKind {
                    field: "category".into(),
                });
            }
            account.cost_centre = cost_centre;
            Ok(())
        })
    }

    pub(super) fn insert_cost_centre(&mut self, name: &str) -> CostCentreId {
        let id = CostCentreId(self.next.cost_centre);
        self.next.cost_centre += 1;
        self.cost_centres.insert(
            id,
            CostCentre {
                id,
                name: name.to_owned(),
            },
        );
        id
    }

    fn unique_centre_name(
        &self,
        name: &str,
        except: Option<CostCentreId>,
    ) -> Result<String, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::EmptyName {
                field: "name".into(),
            });
        }
        let taken = self
            .cost_centres
            .values()
            .any(|c| Some(c.id) != except && c.name.eq_ignore_ascii_case(name));
        if taken {
            return Err(DomainError::DuplicateName {
                field: "name".into(),
            });
        }
        Ok(name.to_owned())
    }
}

fn unknown_centre() -> DomainError {
    DomainError::UnknownCostCentre {
        field: "costCentre".into(),
    }
}

/// Checks the stored cost centres and the categories' references to them.
pub(super) fn check_cost_centres(
    centres: Vec<CostCentre>,
    next: &NextIds,
    accounts: &BTreeMap<AccountId, Account>,
) -> Result<BTreeMap<CostCentreId, CostCentre>, DomainError> {
    let bad = |reason: &str| DomainError::Inconsistent {
        reason: reason.to_owned(),
    };
    let mut out = BTreeMap::new();
    let mut names = BTreeSet::new();
    for c in centres {
        if c.name.trim().is_empty()
            || c.id.0 >= next.cost_centre
            || !names.insert(c.name.to_ascii_lowercase())
        {
            return Err(bad("cost centre with an invalid name or id"));
        }
        out.insert(c.id, c);
    }
    let references_ok = accounts.values().all(|a| {
        a.cost_centre
            .is_none_or(|c| a_category(a.kind) && out.contains_key(&c))
    });
    if !references_ok {
        return Err(bad("category with an invalid cost centre"));
    }
    Ok(out)
}
