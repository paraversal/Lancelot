//! Starting from another event (R-EVT-3, R-ISO-4). Everything is copied once, so the files stay
//! independent, and nothing here refers to the other event afterwards (R-ISO-1).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Book, clean_details};
use crate::error::DomainError;
use crate::model::{
    Account, AccountId, BudgetLine, BudgetLineId, CostCentreId, Event, EventDetails,
};

/// What to copy into an open event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct CopyChoice {
    pub persons: bool,
    pub budget: bool,
}

/// What a copy adds, and what it leaves alone because the event already has it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CopySummary {
    pub persons_added: u32,
    pub persons_skipped: u32,
    pub budget_lines_added: u32,
    pub budget_lines_skipped: u32,
    pub categories_added: u32,
    pub cost_centres_added: u32,
}

/// What a new event gets from a template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSummary {
    pub name: String,
    pub categories: u32,
    pub persons: u32,
    pub budget_lines: u32,
    pub tiers: u32,
    pub drink_products: u32,
}

impl Book {
    /// A new event with the source's setup and none of its money (R-EVT-3): accounts, categories,
    /// cost centres, persons, budget lines, tiers, sheet columns and the drink price list. The
    /// source's own ids carry over, which is safe because ids only mean something within a file.
    pub fn from_template(source: &Book, details: EventDetails) -> Result<Book, DomainError> {
        let details = clean_details(details)?;
        let mut book = source.clone();
        book.event = Event {
            details,
            closed_at: None,
            closing_overrides: Vec::new(),
            ..source.event.clone()
        };
        book.txns.clear();
        book.attachments.clear();
        book.closing_counts.clear();
        book.reconciliations.clear();
        book.imports.clear();
        book.manual_sales.clear();
        book.cancellations.clear();
        for line in book.budget_lines.values_mut() {
            line.draft.complete = false;
        }
        for product in book.drink_products.values_mut() {
            product.draft.taken = 0;
            product.draft.returned = 0;
        }
        book.sync_tickets()?;
        book.recompute()?;
        Ok(book)
    }

    pub fn template_summary(&self) -> TemplateSummary {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        TemplateSummary {
            name: self.event.details.name.clone(),
            categories: count(
                self.accounts
                    .values()
                    .filter(|a| super::a_category(a.kind))
                    .count(),
            ),
            persons: count(self.persons.len()),
            budget_lines: count(self.budget_lines.len()),
            tiers: count(self.tiers.len()),
            drink_products: count(self.drink_products.len()),
        }
    }

    /// Copies persons and budget lines from another event (R-ISO-4). It never overwrites: a person
    /// with the same name and a budget line with the same category and description stay as they
    /// are. A category or cost centre the budget needs is created when it is missing.
    pub fn copy_from(
        &mut self,
        source: &Book,
        choice: CopyChoice,
    ) -> Result<CopySummary, DomainError> {
        self.change(|book| {
            let mut summary = CopySummary::default();
            if choice.persons {
                book.copy_persons(source, &mut summary);
            }
            if choice.budget {
                book.copy_budget(source, &mut summary);
            }
            Ok(summary)
        })
    }

    fn copy_persons(&mut self, source: &Book, summary: &mut CopySummary) {
        for (account, person) in source.persons() {
            if self.account_by_name(account.kind, &account.name).is_some() {
                summary.persons_skipped += 1;
                continue;
            }
            let id = self.insert_account(account.kind, &account.name, None);
            self.persons.insert(id, person.clone());
            summary.persons_added += 1;
        }
    }

    fn copy_budget(&mut self, source: &Book, summary: &mut CopySummary) {
        // Only what was here before: the source may have two lines with the same description.
        let existing: BTreeSet<(AccountId, String)> = self
            .budget_lines
            .values()
            .map(|b| (b.draft.category, b.draft.description.to_lowercase()))
            .collect();
        for line in source.budget_lines.values() {
            let Some(account) = source.accounts.get(&line.draft.category) else {
                continue;
            };
            let category = self.matching_category(source, account, summary);
            if existing.contains(&(category, line.draft.description.to_lowercase())) {
                summary.budget_lines_skipped += 1;
                continue;
            }
            let id = BudgetLineId(self.next.budget_line);
            self.next.budget_line += 1;
            let mut draft = line.draft.clone();
            draft.category = category;
            draft.complete = false;
            draft.cost_centre = line
                .draft
                .cost_centre
                .and_then(|c| self.matching_centre(source, c, summary));
            self.budget_lines.insert(id, BudgetLine { id, draft });
            summary.budget_lines_added += 1;
        }
    }

    /// The category with the same role, else with the same name, else a new one.
    fn matching_category(
        &mut self,
        source: &Book,
        account: &Account,
        summary: &mut CopySummary,
    ) -> AccountId {
        let found = account
            .role
            .and_then(|role| self.account_by_role(role))
            .or_else(|| self.account_by_name(account.kind, &account.name))
            .map(|a| a.id);
        if let Some(id) = found {
            return id;
        }
        let centre = account
            .cost_centre
            .and_then(|c| self.matching_centre(source, c, summary));
        let id = self.insert_account(account.kind, &account.name, account.role);
        if let Some(a) = self.accounts.get_mut(&id) {
            a.cost_centre = centre;
        }
        summary.categories_added += 1;
        id
    }

    /// The cost centre with the same role, else with the same name, else a new one.
    fn matching_centre(
        &mut self,
        source: &Book,
        id: CostCentreId,
        summary: &mut CopySummary,
    ) -> Option<CostCentreId> {
        let centre = source.cost_centres.get(&id)?;
        let found = self
            .cost_centres
            .values()
            .find(|c| {
                (centre.role.is_some() && c.role == centre.role)
                    || c.name.eq_ignore_ascii_case(&centre.name)
            })
            .map(|c| c.id);
        Some(found.unwrap_or_else(|| {
            summary.cost_centres_added += 1;
            self.insert_cost_centre(&centre.name, centre.role)
        }))
    }
}
