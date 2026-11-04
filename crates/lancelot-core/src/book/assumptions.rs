//! The book's assumptions: the numbers a budget is planned on, such as the guests expected
//! (R-BUD-6). A budget line's quantity can scale with them, so changing one changes every line
//! that uses it. Planning data only (R-BUD-5).

use std::collections::{BTreeMap, BTreeSet};

use super::{Book, NextIds};
use crate::error::DomainError;
use crate::model::{Assumption, AssumptionId, Estimate};

impl Book {
    /// In creation order.
    pub fn assumptions(&self) -> impl Iterator<Item = &Assumption> {
        self.assumptions.values()
    }

    pub fn assumption(&self, id: AssumptionId) -> Option<&Assumption> {
        self.assumptions.get(&id)
    }

    pub fn add_assumption(&mut self, name: &str, value: u32) -> Result<AssumptionId, DomainError> {
        self.change(|book| {
            let name = book.unique_assumption_name(name, None)?;
            let id = AssumptionId(book.next.assumption);
            book.next.assumption += 1;
            book.assumptions.insert(id, Assumption { id, name, value });
            Ok(id)
        })
    }

    /// Renames an assumption or gives it another value. Every budget line that scales with it
    /// follows; a value that would take one of them out of range is refused.
    pub fn update_assumption(
        &mut self,
        id: AssumptionId,
        name: &str,
        value: u32,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let name = book.unique_assumption_name(name, Some(id))?;
            let assumption = book.assumptions.get_mut(&id).ok_or_else(unknown)?;
            assumption.name = name;
            assumption.value = value;
            Ok(())
        })
    }

    /// Deletes an assumption that no budget line scales with.
    pub fn delete_assumption(&mut self, id: AssumptionId) -> Result<Assumption, DomainError> {
        self.change(|book| {
            if book.lines_scaling_with(id) > 0 {
                return Err(DomainError::AssumptionInUse);
            }
            book.assumptions.remove(&id).ok_or_else(unknown)
        })
    }

    /// How many budget lines scale with the assumption.
    pub fn lines_scaling_with(&self, id: AssumptionId) -> usize {
        self.budget_lines
            .values()
            .filter(|b| scales_with(&b.draft.estimate, id))
            .count()
    }

    fn unique_assumption_name(
        &self,
        name: &str,
        except: Option<AssumptionId>,
    ) -> Result<String, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::EmptyName {
                field: "name".into(),
            });
        }
        let taken = self
            .assumptions
            .values()
            .any(|a| Some(a.id) != except && a.name.eq_ignore_ascii_case(name));
        if taken {
            return Err(DomainError::DuplicateName {
                field: "name".into(),
            });
        }
        Ok(name.to_owned())
    }
}

fn scales_with(estimate: &Estimate, id: AssumptionId) -> bool {
    matches!(estimate, Estimate::Quantity { per, .. } if per.contains(&id))
}

fn unknown() -> DomainError {
    DomainError::UnknownAssumption {
        field: "assumption".into(),
    }
}

/// An estimate's assumptions exist and appear once each.
pub(super) fn check_per(
    estimate: &Estimate,
    assumptions: &BTreeMap<AssumptionId, Assumption>,
) -> Result<(), DomainError> {
    let Estimate::Quantity { per, .. } = estimate else {
        return Ok(());
    };
    let mut seen = BTreeSet::new();
    for (i, id) in per.iter().enumerate() {
        if !assumptions.contains_key(id) || !seen.insert(*id) {
            return Err(DomainError::UnknownAssumption {
                field: format!("estimate.per[{i}]"),
            });
        }
    }
    Ok(())
}

/// Checks the stored assumptions.
pub(super) fn check_assumptions(
    stored: Vec<Assumption>,
    next: &NextIds,
) -> Result<BTreeMap<AssumptionId, Assumption>, DomainError> {
    let mut out = BTreeMap::new();
    let mut names = BTreeSet::new();
    for a in stored {
        if a.name.trim() != a.name
            || a.name.is_empty()
            || a.id.0 >= next.assumption
            || !names.insert(a.name.to_ascii_lowercase())
        {
            return Err(DomainError::Inconsistent {
                reason: "assumption with an invalid name or id".to_owned(),
            });
        }
        out.insert(a.id, a);
    }
    Ok(out)
}
