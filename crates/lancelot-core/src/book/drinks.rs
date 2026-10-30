//! The book's cost centres and drink records (requirements §5.8).

use std::collections::{BTreeMap, BTreeSet};

use super::{Book, NextIds, a_category, clean};
use crate::drinks::{DrinkProduct, DrinkProductDraft, DrinkSettings, Drinks};
use crate::error::DomainError;
use crate::model::{Account, AccountId, CostCentre, CostCentreId, CostCentreRole, DrinkProductId};

impl Book {
    pub fn cost_centres(&self) -> impl Iterator<Item = &CostCentre> {
        self.cost_centres.values()
    }

    pub fn cost_centre(&self, id: CostCentreId) -> Option<&CostCentre> {
        self.cost_centres.get(&id)
    }

    /// The Drinks cost centre, which every event has (R-CC-2).
    pub fn drinks_cost_centre(&self) -> Option<&CostCentre> {
        self.cost_centres
            .values()
            .find(|c| c.role == Some(CostCentreRole::Drinks))
    }

    pub fn drink_products(&self) -> impl Iterator<Item = &DrinkProduct> {
        self.drink_products.values()
    }

    /// The figures of the records and the invoice check, always up to date.
    pub fn drinks(&self) -> &Drinks {
        &self.drinks
    }

    pub fn add_cost_centre(&mut self, name: &str) -> Result<CostCentreId, DomainError> {
        self.change(|book| {
            let name = book.unique_centre_name(name, None)?;
            Ok(book.insert_cost_centre(&name, None))
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

    /// Deletes a cost centre. What was tagged with it stays, without a tag. Drinks stays.
    pub fn delete_cost_centre(&mut self, id: CostCentreId) -> Result<CostCentre, DomainError> {
        self.change(|book| {
            let centre = book.cost_centres.get(&id).ok_or_else(unknown_centre)?;
            if centre.role.is_some() {
                return Err(DomainError::CostCentreProtected);
            }
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

    pub fn add_drink_product(
        &mut self,
        draft: DrinkProductDraft,
    ) -> Result<DrinkProductId, DomainError> {
        self.change(|book| {
            let draft = clean_product(draft)?;
            let id = DrinkProductId(book.next.drink_product);
            book.next.drink_product += 1;
            book.drink_products.insert(id, DrinkProduct { id, draft });
            Ok(id)
        })
    }

    pub fn update_drink_product(
        &mut self,
        id: DrinkProductId,
        draft: DrinkProductDraft,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let draft = clean_product(draft)?;
            if !book.drink_products.contains_key(&id) {
                return Err(DomainError::UnknownDrinkProduct);
            }
            book.drink_products.insert(id, DrinkProduct { id, draft });
            Ok(())
        })
    }

    pub fn delete_drink_product(
        &mut self,
        id: DrinkProductId,
    ) -> Result<DrinkProduct, DomainError> {
        self.change(|book| {
            book.drink_products
                .remove(&id)
                .ok_or(DomainError::UnknownDrinkProduct)
        })
    }

    pub fn set_drink_settings(&mut self, settings: DrinkSettings) -> Result<(), DomainError> {
        self.change(|book| {
            book.event.drinks = clean_settings(settings)?;
            Ok(())
        })
    }

    pub(super) fn insert_cost_centre(
        &mut self,
        name: &str,
        role: Option<CostCentreRole>,
    ) -> CostCentreId {
        let id = CostCentreId(self.next.cost_centre);
        self.next.cost_centre += 1;
        self.cost_centres.insert(
            id,
            CostCentre {
                id,
                name: name.to_owned(),
                role,
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

fn clean_product(d: DrinkProductDraft) -> Result<DrinkProductDraft, DomainError> {
    let name = d.name.trim().to_owned();
    if name.is_empty() {
        return Err(DomainError::EmptyName {
            field: "name".into(),
        });
    }
    if d.returned > d.taken {
        return Err(DomainError::TooManyReturned);
    }
    if d.unit_price.is_negative() {
        return Err(DomainError::NegativeAmount {
            field: "unitPrice".into(),
        });
    }
    Ok(DrinkProductDraft { name, ..d })
}

pub(super) fn clean_settings(s: DrinkSettings) -> Result<DrinkSettings, DomainError> {
    if s.price_per_strich.is_some_and(|p| p.is_negative()) {
        return Err(DomainError::NegativeAmount {
            field: "pricePerStrich".into(),
        });
    }
    Ok(DrinkSettings {
        supplier: clean(s.supplier),
        ..s
    })
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
    let drinks = out
        .values()
        .filter(|c: &&CostCentre| c.role == Some(CostCentreRole::Drinks))
        .count();
    if drinks != 1 {
        return Err(bad("the Drinks cost centre must exist exactly once"));
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

pub(super) fn check_products(
    products: Vec<DrinkProduct>,
    next: &NextIds,
) -> Result<BTreeMap<DrinkProductId, DrinkProduct>, DomainError> {
    let mut out = BTreeMap::new();
    for p in products {
        if p.id.0 >= next.drink_product || clean_product(p.draft.clone()).as_ref() != Ok(&p.draft) {
            return Err(DomainError::Inconsistent {
                reason: "product with an invalid id, name or count".to_owned(),
            });
        }
        out.insert(p.id, p);
    }
    Ok(out)
}
