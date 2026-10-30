//! Drinks on commission (requirements §5.8): the per-product records, the cost per drink and the
//! check of the supplier invoice. The Drinks P&L itself is the cost centre's, in
//! [`crate::budget::Budget`]. Recomputed on every change, like the [`crate::Ledger`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{
    Account, AccountId, CostCentre, CostCentreId, CostCentreRole, DrinkProductId, Role, Txn, TxnId,
    TxnKind,
};
use crate::money::{Cents, MoneyError, try_sum};

/// The event's settings for drinks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DrinkSettings {
    /// Who the drinks come from: expenses with this counterparty are the supplier's invoice.
    pub supplier: Option<String>,
    /// One price for all drinks (R-DRK-6).
    pub price_per_strich: Option<Cents>,
}

/// One product taken on commission (R-DRK-5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct DrinkProduct {
    pub id: DrinkProductId,
    #[serde(flatten)]
    pub draft: DrinkProductDraft,
}

/// What the product form fills in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DrinkProductDraft {
    pub name: String,
    pub taken: u32,
    /// Never more than `taken`.
    pub returned: u32,
    pub unit_price: Cents,
}

impl DrinkProductDraft {
    pub fn kept(&self) -> u32 {
        self.taken.saturating_sub(self.returned)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct DrinkProductFigures {
    pub product: DrinkProductId,
    pub kept: u32,
    /// Kept units × price per unit.
    pub cost: Cents,
}

/// The supplier's invoice as booked, against the records (R-DRK-5). Pfand is on the invoice but
/// not in the records (R-DRK-2), so it is left out of the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct InvoiceCheck {
    /// Everything booked to the Drinks cost centre with the supplier as counterparty.
    pub amount: Cents,
    /// The part of `amount` in the Pfand category.
    pub pfand: Cents,
    /// `amount` without Pfand, minus the cost of the records. Zero when the invoice is right.
    pub difference: Cents,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Drinks {
    /// In the order of the products' ids.
    pub products: Vec<DrinkProductFigures>,
    pub kept: u32,
    /// What the records say the kept units cost.
    pub cost: Cents,
    /// `cost` per kept unit, rounded half up; none while nothing is kept.
    pub cost_per_drink: Option<Cents>,
    /// None until an invoice from the supplier is booked.
    pub invoice: Option<InvoiceCheck>,
}

impl Drinks {
    pub fn compute(
        products: &BTreeMap<DrinkProductId, DrinkProduct>,
        settings: &DrinkSettings,
        accounts: &BTreeMap<AccountId, Account>,
        cost_centres: &BTreeMap<CostCentreId, CostCentre>,
        txns: &BTreeMap<TxnId, Txn>,
    ) -> Result<Drinks, MoneyError> {
        let mut figures = Vec::new();
        for p in products.values() {
            figures.push(DrinkProductFigures {
                product: p.id,
                kept: p.draft.kept(),
                cost: p.draft.unit_price.checked_mul(i64::from(p.draft.kept()))?,
            });
        }
        let kept = figures
            .iter()
            .try_fold(0u32, |sum, f| sum.checked_add(f.kept))
            .ok_or(MoneyError::Overflow)?;
        let cost = try_sum(figures.iter().map(|f| f.cost))?;
        let cost_per_drink = (kept > 0).then(|| {
            let kept = i128::from(kept);
            let rounded = (i128::from(cost.get()) * 2 + kept) / (kept * 2);
            // At most the highest unit price, so it stays in range.
            Cents::new(rounded as i64)
        });

        let drinks_centre = cost_centres
            .values()
            .find(|c| c.role == Some(CostCentreRole::Drinks))
            .map(|c| c.id);
        let invoice = match (settings.supplier.as_deref(), drinks_centre) {
            (Some(supplier), Some(centre)) => {
                let lines: Vec<_> = txns
                    .values()
                    .filter(|t| {
                        t.kind == TxnKind::Expense
                            && t.counterparty
                                .as_deref()
                                .is_some_and(|c| c.trim().eq_ignore_ascii_case(supplier))
                    })
                    .flat_map(|t| &t.lines)
                    .filter(|l| l.cost_centre == Some(centre))
                    .collect();
                if lines.is_empty() {
                    None
                } else {
                    let amount = try_sum(lines.iter().map(|l| l.amount))?;
                    let pfand = try_sum(
                        lines
                            .iter()
                            .filter(|l| {
                                accounts.get(&l.account).and_then(|a| a.role) == Some(Role::Pfand)
                            })
                            .map(|l| l.amount),
                    )?;
                    Some(InvoiceCheck {
                        amount,
                        pfand,
                        difference: amount.checked_sub(pfand)?.checked_sub(cost)?,
                    })
                }
            }
            _ => None,
        };

        Ok(Drinks {
            products: figures,
            kept,
            cost,
            cost_per_drink: cost_per_drink.transpose()?,
            invoice,
        })
    }

    pub fn product(&self, id: DrinkProductId) -> Option<&DrinkProductFigures> {
        self.products.iter().find(|p| p.product == id)
    }
}
