//! Plan vs actual and the forecast (R-BUD-3, R-BUD-4). Budget lines are planning data only:
//! nothing here feeds back into a balance (R-BUD-5). Recomputed on every change, like the
//! [`crate::Ledger`].

use std::collections::BTreeMap;

use serde::Serialize;
use ts_rs::TS;

use crate::ledger::Ledger;
use crate::model::{
    Account, AccountId, AccountKind, Assumption, AssumptionId, BudgetLine, BudgetLineId,
    CostCentre, CostCentreId, Txn, TxnId,
};
use crate::money::{Cents, MoneyError};

/// Estimate, actual and forecast of a budget line, a category or a total. All three are
/// positive for both income and expenses, except the result, which is income minus expenses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Figures {
    pub estimate: Cents,
    pub actual: Cents,
    pub forecast: Cents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct BudgetLineFigures {
    pub line: BudgetLineId,
    #[serde(flatten)]
    pub figures: Figures,
    /// What this line's forecast does to the planned result: negative when it lowers it.
    #[serde(rename = "fromPlan")]
    pub from_plan: Cents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CategoryFigures {
    pub category: AccountId,
    /// `estimate` sums the category's budget lines; `actual` is everything booked to it.
    #[serde(flatten)]
    pub figures: Figures,
    /// The part of the actual that is linked to no budget line. The forecast adds it on top.
    pub unbudgeted: Cents,
}

/// The profit and loss of a cost centre, with its plan (R-CC-1, R-BUD-3, R-SET-3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CostCentreFigures {
    pub cost_centre: CostCentreId,
    /// The categories that have something tagged to this cost centre, in account order. Each
    /// one's `unbudgeted` is the part of its tagged actual that is linked to no budget line.
    pub categories: Vec<CategoryFigures>,
    pub income: Figures,
    pub expense: Figures,
    /// Income minus expenses.
    pub result: Figures,
}

/// The budget of an event: per line, per category and in total.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Budget {
    /// In the order of the budget lines' ids.
    pub lines: Vec<BudgetLineFigures>,
    /// Every income and expense category, in account order.
    pub categories: Vec<CategoryFigures>,
    pub income: Figures,
    pub expense: Figures,
    /// Income minus expenses. Its actual is the event's result.
    pub result: Figures,
    /// The forecast result minus the planned one (R-BUD-4).
    #[serde(rename = "fromPlan")]
    pub from_plan: Cents,
    /// Every cost centre, in creation order (R-BUD-3).
    #[serde(rename = "costCentres")]
    pub cost_centres: Vec<CostCentreFigures>,
}

impl Budget {
    pub fn compute(
        accounts: &BTreeMap<AccountId, Account>,
        budget_lines: &BTreeMap<BudgetLineId, BudgetLine>,
        assumptions: &BTreeMap<AssumptionId, Assumption>,
        cost_centres: &BTreeMap<CostCentreId, CostCentre>,
        txns: &BTreeMap<TxnId, Txn>,
        ledger: &Ledger,
    ) -> Result<Budget, MoneyError> {
        // Linked amounts, displayed the way their category shows them: positive.
        let mut linked: BTreeMap<BudgetLineId, Cents> = BTreeMap::new();
        for line in txns.values().flat_map(|t| &t.lines) {
            let (Some(id), Some(account)) = (line.budget_line, accounts.get(&line.account)) else {
                continue;
            };
            let amount = line.amount.checked_mul(account.kind.display_sign())?;
            let sum = linked.entry(id).or_default();
            *sum = sum.checked_add(amount)?;
        }

        let mut lines = Vec::new();
        let mut per_category: BTreeMap<AccountId, (Figures, Cents)> = BTreeMap::new();
        let mut per_centre: BTreeMap<(CostCentreId, AccountId), (Figures, Cents)> = BTreeMap::new();
        for b in budget_lines.values() {
            let category = accounts.get(&b.draft.category);
            let kind = category.map(|a| a.kind);
            let estimate = b.draft.estimate.total(assumptions)?;
            let actual = linked.get(&b.id).copied().unwrap_or_default();
            let forecast = if b.draft.complete || category.is_some_and(follows_the_tickets) {
                actual
            } else if kind == Some(AccountKind::Income) {
                // Cautious on both sides: income counts with the smaller figure.
                estimate.min(actual)
            } else {
                estimate.max(actual)
            };
            let figures = Figures {
                estimate,
                actual,
                forecast,
            };
            let from_plan = if kind == Some(AccountKind::Income) {
                forecast.checked_sub(estimate)?
            } else {
                estimate.checked_sub(forecast)?
            };
            lines.push(BudgetLineFigures {
                line: b.id,
                figures,
                from_plan,
            });
            let (sum, budgeted) = per_category.entry(b.draft.category).or_default();
            *sum = add(*sum, figures)?;
            *budgeted = budgeted.checked_add(actual)?;
            if let Some(centre) = b.draft.cost_centre {
                let (sum, budgeted) = per_centre.entry((centre, b.draft.category)).or_default();
                *sum = add(*sum, figures)?;
                *budgeted = budgeted.checked_add(actual)?;
            }
        }

        // Everything tagged, displayed the way its category shows it.
        let mut tagged: BTreeMap<(CostCentreId, AccountId), Cents> = BTreeMap::new();
        for line in txns.values().flat_map(|t| &t.lines) {
            let (Some(centre), Some(account)) = (line.cost_centre, accounts.get(&line.account))
            else {
                continue;
            };
            let amount = line.amount.checked_mul(account.kind.display_sign())?;
            let sum = tagged.entry((centre, line.account)).or_default();
            *sum = sum.checked_add(amount)?;
            per_centre.entry((centre, line.account)).or_default();
        }

        let mut categories = Vec::new();
        let mut income = Figures::default();
        let mut expense = Figures::default();
        for a in accounts.values() {
            let total = match a.kind {
                AccountKind::Income => &mut income,
                AccountKind::Expense => &mut expense,
                _ => continue,
            };
            let (planned, budgeted) = per_category.get(&a.id).copied().unwrap_or_default();
            let actual = ledger.balance(a.id);
            let unbudgeted = actual.checked_sub(budgeted)?;
            let figures = Figures {
                estimate: planned.estimate,
                actual,
                forecast: planned.forecast.checked_add(unbudgeted)?,
            };
            categories.push(CategoryFigures {
                category: a.id,
                figures,
                unbudgeted,
            });
            *total = add(*total, figures)?;
        }

        let result = difference(income, expense)?;
        let mut centres = Vec::new();
        for centre in cost_centres.keys().copied() {
            let mut figures = CostCentreFigures {
                cost_centre: centre,
                categories: Vec::new(),
                income: Figures::default(),
                expense: Figures::default(),
                result: Figures::default(),
            };
            for ((_, category), (planned, budgeted)) in
                per_centre.range((centre, AccountId(i64::MIN))..=(centre, AccountId(i64::MAX)))
            {
                let actual = tagged
                    .get(&(centre, *category))
                    .copied()
                    .unwrap_or_default();
                let unbudgeted = actual.checked_sub(*budgeted)?;
                let line = Figures {
                    estimate: planned.estimate,
                    actual,
                    forecast: planned.forecast.checked_add(unbudgeted)?,
                };
                let total = match accounts.get(category).map(|a| a.kind) {
                    Some(AccountKind::Income) => &mut figures.income,
                    _ => &mut figures.expense,
                };
                *total = add(*total, line)?;
                figures.categories.push(CategoryFigures {
                    category: *category,
                    figures: line,
                    unbudgeted,
                });
            }
            figures.result = difference(figures.income, figures.expense)?;
            centres.push(figures);
        }

        Ok(Budget {
            cost_centres: centres,
            lines,
            categories,
            income,
            expense,
            from_plan: result.forecast.checked_sub(result.estimate)?,
            result,
        })
    }

    pub fn line(&self, id: BudgetLineId) -> Option<&Figures> {
        self.lines.iter().find(|l| l.line == id).map(|l| &l.figures)
    }

    pub fn cost_centre(&self, id: CostCentreId) -> Option<&CostCentreFigures> {
        self.cost_centres.iter().find(|c| c.cost_centre == id)
    }

    pub fn category(&self, id: AccountId) -> Option<&CategoryFigures> {
        self.categories.iter().find(|c| c.category == id)
    }
}

/// The ticket categories are generated from what was sold (R-TKT-6, R-TKT-15). Their forecast is
/// what is booked: an estimate on top would count the same money twice.
fn follows_the_tickets(account: &Account) -> bool {
    account.role.is_some()
}

fn difference(income: Figures, expense: Figures) -> Result<Figures, MoneyError> {
    Ok(Figures {
        estimate: income.estimate.checked_sub(expense.estimate)?,
        actual: income.actual.checked_sub(expense.actual)?,
        forecast: income.forecast.checked_sub(expense.forecast)?,
    })
}

fn add(a: Figures, b: Figures) -> Result<Figures, MoneyError> {
    Ok(Figures {
        estimate: a.estimate.checked_add(b.estimate)?,
        actual: a.actual.checked_add(b.actual)?,
        forecast: a.forecast.checked_add(b.forecast)?,
    })
}
