//! Ticket statistics and profitability (R-TKT-8, R-TKT-9).
//!
//! The statistics read from one source, [`sales_over_time`]. It walks through [`Step`]s, and today
//! those come from the import snapshots. A sale date column would only be another way to build
//! the steps: tickets per order and tier as of each day, in the shape of an import.

use std::collections::BTreeMap;

use serde::Serialize;
use time::Date;
use ts_rs::TS;

use crate::budget::Budget;
use crate::error::DomainError;
use crate::model::{CancellationId, ImportId, ManualSaleId, TierId};
use crate::money::{Cents, MoneyError};
use crate::tickets::{Cancellation, ManualSale, TicketImport, TicketTotals, Tickets, Tier};

/// Where the sales stood on one day, with the tiers in creation order. Money uses today's tier
/// prices, like the generated income.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct SalesPoint {
    #[ts(type = "string")]
    pub date: Date,
    /// The import it stands for. Absent on a last point for manual sales and refunds dated after
    /// the latest import.
    pub import: Option<ImportId>,
    pub tiers: Vec<TierSales>,
    pub total: TicketTotals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TierSales {
    pub tier: TierId,
    #[serde(flatten)]
    pub figures: TicketTotals,
}

/// One moment of the curve: the sheets known by then, the latest one last.
struct Step<'a> {
    date: Date,
    import: Option<ImportId>,
    imports: &'a [&'a TicketImport],
}

/// One step per import, plus a last one when manual sales or refunds came after the latest
/// import, so the curve always ends on today's totals.
fn steps_from_snapshots<'a>(
    imports: &'a [&'a TicketImport],
    sales: &BTreeMap<ManualSaleId, ManualSale>,
    cancellations: &BTreeMap<CancellationId, Cancellation>,
) -> Vec<Step<'a>> {
    let mut steps: Vec<Step> = imports
        .iter()
        .enumerate()
        .map(|(i, import)| Step {
            date: import.date,
            import: Some(import.id),
            imports: &imports[..=i],
        })
        .collect();
    let last_entry = sales
        .values()
        .map(|s| s.draft.date)
        .chain(cancellations.values().filter_map(|c| Some(c.terms?.date)))
        .max();
    if let Some(date) = last_entry
        && imports.last().is_none_or(|i| i.date < date)
    {
        steps.push(Step {
            date,
            import: None,
            imports,
        });
    }
    steps
}

/// Sold, cancelled and money over time, oldest first (R-TKT-8). The last point equals the
/// current totals.
pub(crate) fn sales_over_time(
    tiers: &BTreeMap<TierId, Tier>,
    imports: &[&TicketImport],
    sales: &BTreeMap<ManualSaleId, ManualSale>,
    cancellations: &BTreeMap<CancellationId, Cancellation>,
) -> Result<Vec<SalesPoint>, DomainError> {
    steps_from_snapshots(imports, sales, cancellations)
        .into_iter()
        .map(|step| {
            let (figures, total) = crate::tickets::figures_at(
                tiers,
                step.imports,
                sales,
                cancellations,
                Some(step.date),
            )?;
            Ok(SalesPoint {
                date: step.date,
                import: step.import,
                tiers: figures
                    .into_iter()
                    .map(|(tier, figures)| TierSales { tier, figures })
                    .collect(),
                total,
            })
        })
        .collect()
}

/// The projected result and what it takes to break even (R-TKT-9).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Profitability {
    /// The event's forecast: the tickets sold so far, the other income and the costs (R-BUD-4).
    pub projected: Cents,
    /// How much is missing to reach zero. Zero once the forecast is not negative.
    pub gap: Cents,
    /// About how many more tickets close the gap: at the average net price sold so far, or at the
    /// average of the tiers that bring in something while no sales give an average. Absent when
    /// no tier brings in anything.
    pub tickets: Option<u32>,
    /// Places left over the tiers that bring in something. Absent when one of them has no quota,
    /// or none exists.
    pub places_left: Option<u32>,
    /// In tier order, without tiers that bring in nothing per ticket. Empty without a gap.
    pub tiers: Vec<TierBreakEven>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TierBreakEven {
    pub tier: TierId,
    /// What one more ticket brings in: its price minus the platform's fee.
    pub net_price: Cents,
    /// More tickets of this tier alone that close the gap.
    pub tickets: u32,
    /// Places left in its quota. Fewer than `tickets` means this tier alone cannot do it.
    pub left: Option<u32>,
}

impl Profitability {
    pub(crate) fn compute(tickets: &Tickets, budget: &Budget) -> Result<Profitability, MoneyError> {
        let projected = budget.result.forecast;
        let gap = if projected.is_negative() {
            -projected
        } else {
            Cents::ZERO
        };

        let mut tiers = Vec::new();
        let mut tier_prices = Cents::ZERO;
        let mut paying = 0u32;
        let mut places_left = Some(0u32);
        for t in &tickets.tiers {
            let net_price = t.tier.money(1)?.net;
            if !net_price.is_positive() {
                continue;
            }
            tier_prices = tier_prices.checked_add(net_price)?;
            paying += 1;
            let left = t
                .tier
                .draft
                .quota
                .map(|q| q.saturating_sub(t.figures.places));
            places_left = places_left.zip(left).map(|(sum, l)| sum.saturating_add(l));
            if gap.is_positive() {
                tiers.push(TierBreakEven {
                    tier: t.tier.id,
                    net_price,
                    tickets: needed(gap, net_price, 1),
                    left,
                });
            }
        }

        let net = tickets.total.net;
        let tickets = if tickets.total.sold > 0 && net.is_positive() {
            Some(needed(gap, net, tickets.total.sold))
        } else {
            (paying > 0).then(|| needed(gap, tier_prices, paying))
        };
        Ok(Profitability {
            projected,
            gap,
            tickets,
            places_left: places_left.filter(|_| paying > 0),
            tiers,
        })
    }
}

/// The smallest number of tickets worth at least `gap`, when `count` tickets bring in `net`:
/// ⌈gap × count ÷ net⌉, in integers. `net` must be positive.
fn needed(gap: Cents, net: Cents, count: u32) -> u32 {
    let (gap, net) = (i128::from(gap.get()), i128::from(net.get()));
    let tickets = (gap * i128::from(count) + net - 1) / net;
    u32::try_from(tickets).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(cents: i64) -> Cents {
        Cents::new(cents).unwrap()
    }

    #[test]
    fn break_even_rounds_up_to_whole_tickets() {
        assert_eq!(needed(c(21_200), c(652_750), 214), 7);
        // Exactly enough needs no extra ticket.
        assert_eq!(needed(c(6_400), c(3_200), 1), 2);
        assert_eq!(needed(c(6_401), c(3_200), 1), 3);
        assert_eq!(needed(Cents::ZERO, c(3_200), 1), 0);
    }
}
