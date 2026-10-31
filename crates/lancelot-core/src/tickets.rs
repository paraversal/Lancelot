//! Ticket sales (requirements §5.6): tiers, imports of the ticket sheet and manual sales, and
//! what they add up to.
//!
//! The sheet is the source of truth for ticket income (R-TKT-11). Each import is kept as a
//! snapshot of its orders, which gives the sales over time ([`crate::sales`]), and tickets that
//! left the sheet become conflicts (R-TKT-12). Imports count in the order of their dates, so a
//! file imported after the fact takes its place in the past. Conflicts are derived from the imports; only
//! their resolutions are stored, as [`Cancellation`]s. The income and the refunds are generated
//! by [`crate::Book`] (R-TKT-6, R-TKT-7, R-TKT-15).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use ts_rs::TS;

use crate::error::DomainError;
use crate::model::{
    AccountId, CancellationId, ImportId, ManualSaleId, OpenItem, Origin, TierId, TxnId,
};
use crate::money::{BP_FULL, Cents, MoneyError, percent_bp, try_sum};
use crate::sales::{SalesPoint, sales_over_time};
use crate::sheet::{RowProblem, Sheet};

/// The sheet's column names (R-TKT-3), saved in the event. Matched ignoring case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TicketColumns {
    pub tier: String,
    pub order: String,
    /// Not read yet: the sales over time switch to sale dates once the sheet has them (R-TKT-8).
    pub sale_date: Option<String>,
}

impl Default for TicketColumns {
    fn default() -> TicketColumns {
        TicketColumns {
            tier: "Ticket".into(),
            order: "Order ID".into(),
            sale_date: None,
        }
    }
}

/// Whether the event sells tickets at all, and where the sheet's money arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct TicketSettings {
    pub enabled: bool,
    /// An asset account. Needed only to import.
    pub account: Option<AccountId>,
}

/// A kind of ticket with its price (R-TKT-1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct Tier {
    pub id: TierId,
    #[serde(flatten)]
    pub draft: TierDraft,
}

/// What the tier form fills in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TierDraft {
    pub name: String,
    /// The gross price of one ticket.
    pub price: Cents,
    /// The platform's fee in basis points: `250` is 2,5 % (R-TKT-10).
    pub fee_bp: u32,
    pub quota: Option<u32>,
    /// Other names the sheet uses for this tier (R-TKT-2). The tier's own name always matches.
    pub aliases: Vec<String>,
}

/// What a number of tickets of a tier come to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TicketMoney {
    pub gross: Cents,
    pub fees: Cents,
    pub net: Cents,
}

/// A percentage of `count` tickets and what remains of their price: a fee or a refund.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TicketShare {
    pub part: Cents,
    pub rest: Cents,
}

/// The one rule for a fee or refund (R-COR-6): rounded half-up to the cent on each ticket
/// (R-TKT-10, R-TKT-15). The forms preview it through a command.
pub fn ticket_share(price: Cents, bp: u32, count: u32) -> Result<TicketShare, MoneyError> {
    let count = i64::from(count);
    let part = percent_bp(price, bp)?.checked_mul(count)?;
    Ok(TicketShare {
        part,
        rest: price.checked_mul(count)?.checked_sub(part)?,
    })
}

impl Tier {
    /// The money of `count` tickets. Every figure and generated entry of a ticket comes from here.
    pub fn money(&self, count: u32) -> Result<TicketMoney, MoneyError> {
        let share = ticket_share(self.draft.price, self.draft.fee_bp, count)?;
        Ok(TicketMoney {
            gross: self.draft.price.checked_mul(i64::from(count))?,
            fees: share.part,
            net: share.rest,
        })
    }

    /// The refund on `count` tickets.
    pub fn refund(&self, refund_bp: u32, count: u32) -> Result<Cents, MoneyError> {
        Ok(ticket_share(self.draft.price, refund_bp, count)?.part)
    }

    /// Whether `name` from the sheet means this tier: its name or one of its aliases,
    /// ignoring case and surrounding spaces.
    pub fn matches(&self, name: &str) -> bool {
        std::iter::once(&self.draft.name)
            .chain(&self.draft.aliases)
            .any(|n| same_name(n, name))
    }
}

pub(crate) fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// One import of the ticket sheet (R-TKT-5). The file itself is not stored (R-TKT-17).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TicketImport {
    pub id: ImportId,
    pub filename: String,
    /// SHA-256 of the file, lower-case hex.
    pub sha256: String,
    #[serde(with = "time::serde::rfc3339")]
    #[ts(type = "string")]
    pub imported_at: OffsetDateTime,
    /// The day the counts stand for, the import day unless changed (R-TKT-8).
    #[ts(type = "string")]
    pub date: Date,
    /// Tickets per order and tier, sorted. Order IDs are the only buyer data kept (R-TKT-18);
    /// they stay out of the change log, and the snapshot only has those of conflicts and
    /// cancellations.
    #[serde(skip)]
    #[ts(skip)]
    pub orders: Vec<OrderCount>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderCount {
    pub order: String,
    pub tier: TierId,
    pub count: u32,
}

impl TicketImport {
    /// Tickets per tier.
    pub fn counts(&self) -> BTreeMap<TierId, u32> {
        let mut out = BTreeMap::new();
        for o in &self.orders {
            *out.entry(o.tier).or_insert(0) += o.count;
        }
        out
    }
}

/// Tickets sold outside the sheet, e.g. at the door (R-TKT-7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct ManualSale {
    pub id: ManualSaleId,
    #[serde(flatten)]
    pub draft: ManualSaleDraft,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct ManualSaleDraft {
    pub tier: TierId,
    pub count: u32,
    /// The account or person who received the money.
    pub destination: AccountId,
    #[ts(type = "string")]
    pub date: Date,
    pub reference: Option<String>,
}

/// A cancelled ticket, or a correction of the sheet (R-TKT-13, R-TKT-14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Cancellation {
    pub id: CancellationId,
    pub tier: TierId,
    pub count: u32,
    /// The sheet order it belongs to, unless it belongs to a manual sale. Order IDs stay out of
    /// the change log (R-TKT-18); the view carries them to the screen.
    #[serde(skip)]
    #[ts(skip)]
    pub order: Option<String>,
    pub sale: Option<ManualSaleId>,
    /// How many of its tickets were confirmed as gone from the sheet. The rest are still in it,
    /// like a partial refund on an order that stays (R-TKT-14).
    pub gone: u32,
    pub note: Option<String>,
    /// `None` for a correction: the ticket was never really sold.
    pub terms: Option<RefundTerms>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RefundTerms {
    /// In basis points: `10000` is the whole price back, `0` keeps it all.
    pub refund_bp: u32,
    #[ts(type = "string")]
    pub date: Date,
    /// Who pays. `None`: whoever received the money, the ticket account for the sheet and the
    /// destination of a manual sale (R-TKT-15).
    pub payer: Option<AccountId>,
}

/// A cancellation recorded without a change in the sheet (R-TKT-14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct CancellationDraft {
    pub source: CancellationSource,
    pub count: u32,
    pub terms: RefundTerms,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum CancellationSource {
    Order { order: String, tier: TierId },
    Sale { sale: ManualSaleId },
}

/// What can change on a recorded cancellation: its tickets stay where they are.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
pub struct CancellationEdit {
    pub count: u32,
    pub terms: RefundTerms,
    pub note: Option<String>,
}

/// How a conflict is resolved, wholly or in part (R-TKT-13).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS, Deserialize)]
#[ts(export)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Resolution {
    /// A cancellation with a refund of 0–100 %.
    Refund {
        count: u32,
        terms: RefundTerms,
        note: Option<String>,
    },
    /// The tickets of a cancellation recorded earlier (R-TKT-14).
    Match {
        cancellation: CancellationId,
        count: u32,
    },
    /// The tickets were never really sold: no income and no refund.
    Correction { count: u32, note: Option<String> },
}

/// Tickets per (order ID, tier) from a sheet, and the tier names that match no tier.
pub(crate) struct Orders {
    pub orders: Vec<OrderCount>,
    /// Unknown tier names as first spelled in the sheet, with their number of rows.
    pub unknown: Vec<UnknownTier>,
}

pub(crate) fn orders(sheet: &Sheet, tiers: &BTreeMap<TierId, Tier>) -> Orders {
    let mut counts: BTreeMap<(String, TierId), u32> = BTreeMap::new();
    let mut unknown: Vec<UnknownTier> = Vec::new();
    for row in &sheet.rows {
        match tiers.values().find(|t| t.matches(&row.tier)) {
            Some(t) => *counts.entry((row.order.clone(), t.id)).or_insert(0) += 1,
            None => match unknown.iter_mut().find(|u| same_name(&u.name, &row.tier)) {
                Some(u) => u.rows += 1,
                None => unknown.push(UnknownTier {
                    name: row.tier.clone(),
                    rows: 1,
                }),
            },
        }
    }
    Orders {
        orders: counts
            .into_iter()
            .map(|((order, tier), count)| OrderCount { order, tier, count })
            .collect(),
        unknown,
    }
}

/// What an import would change, shown before it is confirmed (R-TKT-4, R-TKT-5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub filename: String,
    pub sha256: String,
    /// The same file as the import the changes are counted from.
    pub same_file: bool,
    /// The import the changes are counted from: the latest one up to the snapshot date.
    pub previous: Option<ImportId>,
    /// Whether it becomes the sheet the income comes from. If an import is dated later, that one
    /// stays current and this one only adds to the sales over time.
    pub current: bool,
    /// Tickets in the sheet that belong to a known tier.
    pub tickets: u32,
    /// Tier names that match no tier or alias. They block the import until mapped.
    pub unknown: Vec<UnknownTier>,
    /// Rows that could not be read.
    pub problems: Vec<RowProblem>,
    /// Every tier that has tickets before or after, in tier order.
    pub tiers: Vec<TierChange>,
    /// Orders with fewer tickets of a tier than before (R-TKT-12).
    pub removed: Vec<OrderChange>,
    /// Receipts on generated income that would go with it, by file name.
    pub receipts_lost: Vec<String>,
}

/// What deleting an import would change. It goes back to the sheets that are left.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ImportDeletion {
    pub sold_before: u32,
    pub sold_after: u32,
    pub net_before: Cents,
    pub net_after: Cents,
    pub conflicts_before: u32,
    pub conflicts_after: u32,
    /// Receipts on generated income that would go with it, by file name.
    pub receipts_lost: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct UnknownTier {
    pub name: String,
    pub rows: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TierChange {
    pub tier: TierId,
    pub before: u32,
    pub after: u32,
    /// New tickets, counted per order: a tier can gain and lose tickets in one import.
    pub added: u32,
    pub removed: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OrderChange {
    pub order: String,
    pub tier: TierId,
    pub before: u32,
    pub after: u32,
}

pub(crate) fn preview(
    sheet: &Sheet,
    tiers: &BTreeMap<TierId, Tier>,
    previous: Option<&TicketImport>,
    current: bool,
) -> ImportPreview {
    let Orders { orders, unknown } = orders(sheet, tiers);
    let key = |o: &OrderCount| (o.order.clone(), o.tier);
    let before: BTreeMap<_, u32> = previous
        .map(|p| p.orders.iter().map(|o| (key(o), o.count)).collect())
        .unwrap_or_default();
    let after: BTreeMap<_, u32> = orders.iter().map(|o| (key(o), o.count)).collect();

    let mut changes: BTreeMap<TierId, TierChange> = BTreeMap::new();
    let mut removed = Vec::new();
    let keys: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
    for k in keys {
        let (b, a) = (
            before.get(k).copied().unwrap_or(0),
            after.get(k).copied().unwrap_or(0),
        );
        let (order, tier) = k;
        let c = changes.entry(*tier).or_insert(TierChange {
            tier: *tier,
            before: 0,
            after: 0,
            added: 0,
            removed: 0,
        });
        c.before += b;
        c.after += a;
        c.added += a.saturating_sub(b);
        c.removed += b.saturating_sub(a);
        if a < b {
            removed.push(OrderChange {
                order: order.clone(),
                tier: *tier,
                before: b,
                after: a,
            });
        }
    }
    ImportPreview {
        filename: sheet.filename.clone(),
        sha256: sheet.sha256.clone(),
        same_file: previous.is_some_and(|p| p.sha256 == sheet.sha256),
        previous: previous.map(|p| p.id),
        current,
        tickets: orders.iter().map(|o| o.count).sum(),
        unknown,
        problems: sheet.problems.clone(),
        tiers: changes.into_values().collect(),
        removed,
        receipts_lost: Vec::new(),
    }
}

/// Checks a tier against the others: a name, a price, a fee below the price, a positive quota,
/// and names and aliases that no other tier uses. Aliases are trimmed and deduplicated.
pub(crate) fn clean_tier(
    d: TierDraft,
    id: Option<TierId>,
    tiers: &BTreeMap<TierId, Tier>,
) -> Result<TierDraft, DomainError> {
    let name = d.name.trim().to_owned();
    if name.is_empty() {
        return Err(DomainError::EmptyName {
            field: "name".into(),
        });
    }
    if d.price.is_negative() {
        return Err(DomainError::NegativeAmount {
            field: "price".into(),
        });
    }
    let fee = percent_bp(d.price, d.fee_bp)?;
    if d.fee_bp >= BP_FULL || (d.price.is_positive() && fee >= d.price) {
        return Err(DomainError::FeeTooHigh);
    }
    if d.quota == Some(0) {
        return Err(DomainError::NonPositiveAmount {
            field: "quota".into(),
        });
    }
    let mut aliases: Vec<String> = Vec::new();
    for (i, alias) in d.aliases.iter().enumerate() {
        let alias = alias.trim();
        if alias.is_empty() {
            return Err(DomainError::EmptyName {
                field: format!("aliases[{i}]"),
            });
        }
        if !same_name(alias, &name) && !aliases.iter().any(|a| same_name(a, alias)) {
            aliases.push(alias.to_owned());
        }
    }
    let draft = TierDraft { name, aliases, ..d };
    for other in tiers.values().filter(|t| Some(t.id) != id) {
        if other.matches(&draft.name) {
            return Err(DomainError::DuplicateName {
                field: "name".into(),
            });
        }
        if let Some(i) = d.aliases.iter().position(|a| other.matches(a)) {
            return Err(DomainError::DuplicateName {
                field: format!("aliases[{i}]"),
            });
        }
        if draft
            .aliases
            .iter()
            .any(|a| same_name(a, &other.draft.name))
        {
            return Err(DomainError::DuplicateName {
                field: "aliases".into(),
            });
        }
    }
    Ok(draft)
}

/// The imports in the order they count in: by date, then by id. The last one is the current one.
pub(crate) fn by_date(imports: &BTreeMap<ImportId, TicketImport>) -> Vec<&TicketImport> {
    let mut ordered: Vec<&TicketImport> = imports.values().collect();
    ordered.sort_by_key(|i| (i.date, i.id));
    ordered
}

/// Tickets that left the sheet, per order and tier, set against their resolutions.
pub(crate) struct Reckoning {
    /// Unresolved tickets per order and tier, and the import that last removed some.
    pub unresolved: BTreeMap<(String, TierId), (u32, ImportId)>,
    /// How many tickets of each cancellation really are gone from the sheet now. It is less than
    /// the stored `gone` once tickets return, which then count as in the sheet again.
    pub gone: BTreeMap<CancellationId, u32>,
    /// Cancelled tickets no longer in the sheet per tier: still sold (R-TKT-15).
    pub cancelled: BTreeMap<TierId, u32>,
    /// Sold tickets per tier whose place is free again: every cancelled one, whether it left the
    /// sheet or not and whatever the refund. Corrections were never sold.
    pub freed: BTreeMap<TierId, u32>,
}

/// An order's tickets of a tier are missing when it has fewer than it ever had, so tickets that
/// come back fill the gap first and a growing tier hides nothing (R-TKT-12). `imports` come in
/// the order of [`by_date`]. With `until`, a place is free only from its cancellation's date on.
pub(crate) fn reckon(
    imports: &[&TicketImport],
    cancellations: &BTreeMap<CancellationId, Cancellation>,
    until: Option<Date>,
) -> Reckoning {
    // (most ever, now, the import that last lowered it)
    let mut seen: BTreeMap<(&str, TierId), (u32, u32, ImportId)> = BTreeMap::new();
    for import in imports {
        let now: BTreeMap<(&str, TierId), u32> = import
            .orders
            .iter()
            .map(|o| ((o.order.as_str(), o.tier), o.count))
            .collect();
        for (key, s) in &mut seen {
            let count = now.get(key).copied().unwrap_or(0);
            if count < s.1 {
                s.2 = import.id;
            }
            s.1 = count;
        }
        for (key, count) in now {
            let s = seen.entry(key).or_insert((0, 0, import.id));
            s.0 = s.0.max(count);
            s.1 = count;
        }
    }

    let mut left: BTreeMap<(&str, TierId), u32> = seen
        .iter()
        .map(|(key, s)| (*key, s.0 - s.1))
        .filter(|(_, missing)| *missing > 0)
        .collect();
    let mut in_sheet: BTreeMap<(&str, TierId), u32> =
        seen.iter().map(|(key, s)| (*key, s.1)).collect();
    let mut gone = BTreeMap::new();
    let mut cancelled: BTreeMap<TierId, u32> = BTreeMap::new();
    let mut freed: BTreeMap<TierId, u32> = BTreeMap::new();
    for c in cancellations.values() {
        let mut count = 0;
        if let Some(order) = &c.order
            && let Some(missing) = left.get_mut(&(order.as_str(), c.tier))
        {
            count = c.gone.min(*missing);
            *missing -= count;
        }
        gone.insert(c.id, count);
        if c.terms.is_some() && count > 0 {
            *cancelled.entry(c.tier).or_insert(0) += count;
        }
        if c.terms.is_some_and(|t| until.is_none_or(|d| t.date <= d)) {
            // Tickets it holds in the sheet, unless the order no longer has them. On a manual
            // sale, all of them.
            let staying = match &c.order {
                Some(order) => in_sheet
                    .get_mut(&(order.as_str(), c.tier))
                    .map_or(0, |held| {
                        let staying = (c.count - count).min(*held);
                        *held -= staying;
                        staying
                    }),
                None => c.count,
            };
            *freed.entry(c.tier).or_insert(0) += count + staying;
        }
    }
    let unresolved = left
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|((order, tier), count)| {
            let since = seen[&(order, tier)].2;
            ((order.to_owned(), tier), (count, since))
        })
        .collect();
    Reckoning {
        unresolved,
        gone,
        cancelled,
        freed,
    }
}

/// The most tickets of a tier an order ever had. Zero when the sheet never had them.
pub(crate) fn most_ever(
    imports: &BTreeMap<ImportId, TicketImport>,
    order: &str,
    tier: TierId,
) -> u32 {
    imports
        .values()
        .flat_map(|i| &i.orders)
        .filter(|o| o.tier == tier && o.order == order)
        .map(|o| o.count)
        .max()
        .unwrap_or(0)
}

/// An order with tickets that left the sheet and have no resolution yet (R-TKT-13).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OrderConflict {
    pub order: String,
    /// In tier order.
    pub removed: Vec<RemovedTickets>,
    /// Tickets of other tiers this order gained meanwhile: together with a removal, a changed
    /// tier, normally resolved as a correction (R-TKT-12).
    pub added: Vec<TierCount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct RemovedTickets {
    pub tier: TierId,
    /// Still to resolve.
    pub count: u32,
    /// The import that last removed some.
    pub since: ImportId,
    /// Cancellations recorded earlier for this order and tier (R-TKT-14).
    pub matches: Vec<ConflictMatch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ConflictMatch {
    pub cancellation: CancellationId,
    /// How many of the removed tickets it covers.
    pub count: u32,
    pub refund_bp: u32,
}

/// A cancellation with what it comes to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CancellationView {
    #[serde(flatten)]
    pub cancellation: Cancellation,
    pub order: Option<String>,
    /// Its tickets that really are gone from the sheet now.
    pub gone_now: u32,
    /// Refund per ticket, rounded half-up to the cent (R-TKT-15).
    pub per_ticket: Cents,
    /// Count × per ticket.
    pub refund: Cents,
    /// Who pays, also when nobody was chosen. Absent for a correction.
    pub payer: Option<AccountId>,
    /// The generated refund, absent when there is nothing to pay.
    pub txn: Option<TxnId>,
}

/// Every ticket figure the UI shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Tickets {
    /// In creation order.
    pub tiers: Vec<TierFigures>,
    pub total: TicketTotals,
    /// Oldest first; the last one is the current one.
    pub imports: Vec<ImportSummary>,
    /// By date, then by id.
    pub manual_sales: Vec<ManualSaleView>,
    /// By order ID.
    pub conflicts: Vec<OrderConflict>,
    /// Oldest first.
    pub cancellations: Vec<CancellationView>,
    /// Sales over time, oldest first (R-TKT-8).
    pub sales: Vec<SalesPoint>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TierFigures {
    #[serde(flatten)]
    pub tier: Tier,
    #[serde(flatten)]
    pub figures: TicketTotals,
}

/// Tickets and money of one tier or of all of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TicketTotals {
    /// In the latest import.
    pub sheet: u32,
    /// Cancelled and no longer in the sheet. Still sold; refunds are apart (R-TKT-15).
    pub cancelled: u32,
    /// Sold manually.
    pub manual: u32,
    /// All three together.
    pub sold: u32,
    /// Places taken against the quota: sold minus every cancelled ticket, also the ones still
    /// in the sheet or on a manual sale.
    pub places: u32,
    /// Removed from the sheet without a resolution: not sold for now (R-TKT-16).
    pub unresolved: u32,
    /// Sold × price.
    pub gross: Cents,
    /// The platform's fees on the sheet's tickets. They are not refunded (R-TKT-15).
    pub fees: Cents,
    /// All refunds of the tier.
    pub refunds: Cents,
    /// Gross minus fees and refunds.
    pub net: Cents,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ImportSummary {
    #[serde(flatten)]
    pub import: TicketImport,
    pub tickets: u32,
    /// Tickets per tier, in tier order; tiers without tickets are left out.
    pub counts: Vec<TierCount>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TierCount {
    pub tier: TierId,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ManualSaleView {
    #[serde(flatten)]
    pub sale: ManualSale,
    pub gross: Cents,
    /// The generated income, absent for free tickets.
    pub txn: Option<TxnId>,
}

impl Tickets {
    /// Conflicts per order and tier: what the navigation badge and the open item count.
    pub fn conflict_count(&self) -> u32 {
        self.conflicts.iter().map(|c| c.removed.len() as u32).sum()
    }

    /// What the tickets keep open when closing the event (R-TKT-16).
    pub fn open_item(&self) -> Option<OpenItem> {
        let count = self.conflict_count();
        (count > 0).then_some(OpenItem::TicketConflicts { count })
    }

    pub(crate) fn compute(
        tiers: &BTreeMap<TierId, Tier>,
        imports_by_id: &BTreeMap<ImportId, TicketImport>,
        sales: &BTreeMap<ManualSaleId, ManualSale>,
        cancellations: &BTreeMap<CancellationId, Cancellation>,
        generated: &BTreeMap<Origin, TxnId>,
        ticket_account: Option<AccountId>,
    ) -> Result<Tickets, DomainError> {
        let imports = by_date(imports_by_id);
        let reckoning = reckon(&imports, cancellations, None);

        let mut views = Vec::new();
        for c in cancellations.values() {
            let refund_on = |count| match (tiers.get(&c.tier), &c.terms) {
                (Some(t), Some(r)) => t.refund(r.refund_bp, count),
                _ => Ok(Cents::ZERO),
            };
            let per_ticket = refund_on(1)?;
            let refund = refund_on(c.count)?;
            views.push(CancellationView {
                order: c.order.clone(),
                gone_now: reckoning.gone.get(&c.id).copied().unwrap_or(0),
                per_ticket,
                refund,
                payer: c
                    .terms
                    .as_ref()
                    .and_then(|r| r.payer.or(default_payer(c, sales, ticket_account))),
                txn: generated
                    .get(&Origin::TicketRefund { cancellation: c.id })
                    .copied(),
                cancellation: c.clone(),
            });
        }

        let (per_tier, total) = figures_at(tiers, &imports, sales, cancellations, None)?;
        let figures = per_tier
            .into_iter()
            .filter_map(|(id, figures)| {
                let tier = tiers.get(&id)?.clone();
                Some(TierFigures { tier, figures })
            })
            .collect();
        let conflicts = order_conflicts(&reckoning, &imports, cancellations);
        let sales_points = sales_over_time(tiers, &imports, sales, cancellations)?;

        let imports = imports
            .iter()
            .map(|i| {
                let counts: Vec<TierCount> = i
                    .counts()
                    .into_iter()
                    .map(|(tier, count)| TierCount { tier, count })
                    .collect();
                ImportSummary {
                    import: (*i).clone(),
                    tickets: counts.iter().map(|c| c.count).sum(),
                    counts,
                }
            })
            .collect();

        let mut manual_sales = Vec::new();
        for s in sales.values() {
            let money = tiers
                .get(&s.draft.tier)
                .map(|t| t.money(s.draft.count))
                .transpose()?;
            manual_sales.push(ManualSaleView {
                sale: s.clone(),
                gross: money.map_or(Cents::ZERO, |m| m.gross),
                txn: generated.get(&Origin::TicketSale { sale: s.id }).copied(),
            });
        }
        manual_sales.sort_by_key(|s| (s.sale.draft.date, s.sale.id));

        Ok(Tickets {
            tiers: figures,
            total,
            imports,
            manual_sales,
            conflicts,
            cancellations: views,
            sales: sales_points,
        })
    }
}

/// Tickets and money per tier, in creation order, and in total, with the sheets known at one
/// moment: `imports` in the order of [`by_date`], and manual sales and refunds up to `until`.
pub(crate) fn figures_at(
    tiers: &BTreeMap<TierId, Tier>,
    imports: &[&TicketImport],
    sales: &BTreeMap<ManualSaleId, ManualSale>,
    cancellations: &BTreeMap<CancellationId, Cancellation>,
    until: Option<Date>,
) -> Result<(Vec<(TierId, TicketTotals)>, TicketTotals), DomainError> {
    let by_then = |date: Date| until.is_none_or(|d| date <= d);
    let reckoning = reckon(imports, cancellations, until);
    let sheet = imports.last().map(|i| i.counts()).unwrap_or_default();
    let mut manual: BTreeMap<TierId, u32> = BTreeMap::new();
    for s in sales.values().filter(|s| by_then(s.draft.date)) {
        *manual.entry(s.draft.tier).or_insert(0) += s.draft.count;
    }
    let mut unresolved: BTreeMap<TierId, u32> = BTreeMap::new();
    for ((_, tier), (count, _)) in &reckoning.unresolved {
        *unresolved.entry(*tier).or_insert(0) += count;
    }
    let mut refunds: BTreeMap<TierId, Cents> = BTreeMap::new();
    for c in cancellations.values() {
        let Some(terms) = c.terms.filter(|t| by_then(t.date)) else {
            continue;
        };
        let refund = match tiers.get(&c.tier) {
            Some(t) => t.refund(terms.refund_bp, c.count)?,
            None => Cents::ZERO,
        };
        let sum = refunds.entry(c.tier).or_default();
        *sum = sum.checked_add(refund)?;
    }

    let mut figures = Vec::new();
    for t in tiers.values() {
        let in_sheet = sheet.get(&t.id).copied().unwrap_or(0);
        let cancelled = reckoning.cancelled.get(&t.id).copied().unwrap_or(0);
        let manual = manual.get(&t.id).copied().unwrap_or(0);
        let sold = in_sheet + cancelled + manual;
        // Manual sales pay no fee, so the two come from different counts.
        let gross = t.money(sold)?.gross;
        let fees = t.money(in_sheet + cancelled)?.fees;
        let refunds = refunds.get(&t.id).copied().unwrap_or_default();
        figures.push((
            t.id,
            TicketTotals {
                sheet: in_sheet,
                cancelled,
                manual,
                sold,
                // A cancellation can be dated before its manual sale.
                places: sold.saturating_sub(reckoning.freed.get(&t.id).copied().unwrap_or(0)),
                unresolved: unresolved.get(&t.id).copied().unwrap_or(0),
                gross,
                fees,
                refunds,
                net: gross.checked_sub(fees)?.checked_sub(refunds)?,
            },
        ));
    }
    let sum = |f: fn(&TicketTotals) -> Cents| try_sum(figures.iter().map(|(_, t)| f(t)));
    let count = |f: fn(&TicketTotals) -> u32| figures.iter().map(|(_, t)| f(t)).sum();
    let total = TicketTotals {
        sheet: count(|t| t.sheet),
        cancelled: count(|t| t.cancelled),
        manual: count(|t| t.manual),
        sold: count(|t| t.sold),
        places: count(|t| t.places),
        unresolved: count(|t| t.unresolved),
        gross: sum(|t| t.gross)?,
        fees: sum(|t| t.fees)?,
        refunds: sum(|t| t.refunds)?,
        net: sum(|t| t.net)?,
    };
    Ok((figures, total))
}

/// What the sheets had brought in, net of fees, from each import on: oldest first, by the import's
/// date. Refunds are apart, with their own dates.
pub(crate) fn sheet_income(
    tiers: &BTreeMap<TierId, Tier>,
    imports: &BTreeMap<ImportId, TicketImport>,
    cancellations: &BTreeMap<CancellationId, Cancellation>,
) -> Result<Vec<(Date, Cents)>, DomainError> {
    let imports = by_date(imports);
    (0..imports.len())
        .map(|i| {
            let date = imports[i].date;
            let (_, total) = figures_at(
                tiers,
                &imports[..=i],
                &BTreeMap::new(),
                cancellations,
                Some(date),
            )?;
            Ok((date, total.gross.checked_sub(total.fees)?))
        })
        .collect()
}

/// Who pays a refund nobody chose a payer for: whoever received the money (R-TKT-15).
pub(crate) fn default_payer(
    c: &Cancellation,
    sales: &BTreeMap<ManualSaleId, ManualSale>,
    ticket_account: Option<AccountId>,
) -> Option<AccountId> {
    match c.sale {
        Some(sale) => sales.get(&sale).map(|s| s.draft.destination),
        None => ticket_account,
    }
}

/// Groups what is unresolved by order, with the cancellations waiting for it and the tiers the
/// order gained meanwhile.
fn order_conflicts(
    reckoning: &Reckoning,
    imports: &[&TicketImport],
    cancellations: &BTreeMap<CancellationId, Cancellation>,
) -> Vec<OrderConflict> {
    let mut conflicts: BTreeMap<&str, OrderConflict> = BTreeMap::new();
    for ((order, tier), (count, since)) in &reckoning.unresolved {
        let mut left = *count;
        let mut matches = Vec::new();
        for c in cancellations.values() {
            let free = c.count - c.gone;
            let Some(terms) = c.terms.as_ref().filter(|_| {
                free > 0 && left > 0 && c.tier == *tier && c.order.as_ref() == Some(order)
            }) else {
                continue;
            };
            let count = free.min(left);
            left -= count;
            matches.push(ConflictMatch {
                cancellation: c.id,
                count,
                refund_bp: terms.refund_bp,
            });
        }
        conflicts
            .entry(order)
            .or_insert_with(|| OrderConflict {
                order: order.clone(),
                removed: Vec::new(),
                added: Vec::new(),
            })
            .removed
            .push(RemovedTickets {
                tier: *tier,
                count: *count,
                since: *since,
                matches,
            });
    }

    let latest = imports.last().copied();
    for (order, conflict) in &mut conflicts {
        // The import before the one that last removed something.
        let since = conflict
            .removed
            .iter()
            .filter_map(|r| imports.iter().position(|i| i.id == r.since))
            .max();
        let before = since.and_then(|at| at.checked_sub(1)).map(|at| imports[at]);
        let of = |i: Option<&TicketImport>, tier: TierId| {
            i.into_iter()
                .flat_map(|i| &i.orders)
                .find(|o| o.tier == tier && o.order == *order)
                .map_or(0, |o| o.count)
        };
        let tiers: BTreeSet<TierId> = latest
            .into_iter()
            .flat_map(|i| &i.orders)
            .filter(|o| o.order == *order)
            .map(|o| o.tier)
            .collect();
        conflict.added = tiers
            .into_iter()
            .filter(|tier| !conflict.removed.iter().any(|r| r.tier == *tier))
            .filter_map(|tier| {
                let count = of(latest, tier).saturating_sub(of(before, tier));
                (count > 0).then_some(TierCount { tier, count })
            })
            .collect();
    }
    conflicts.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::SheetRow;

    fn tier(id: i64, name: &str, aliases: &[&str]) -> (TierId, Tier) {
        let id = TierId(id);
        let tier = Tier {
            id,
            draft: TierDraft {
                name: name.into(),
                price: Cents::new(3200).unwrap(),
                fee_bp: 0,
                quota: None,
                aliases: aliases.iter().map(|a| (*a).into()).collect(),
            },
        };
        (id, tier)
    }

    fn sheet(rows: &[(&str, &str)]) -> Sheet {
        Sheet {
            filename: "tickets.ods".into(),
            sha256: "ab".into(),
            rows: rows
                .iter()
                .zip(2..)
                .map(|((tier, order), row)| SheetRow {
                    row,
                    tier: (*tier).into(),
                    order: (*order).into(),
                })
                .collect(),
            problems: Vec::new(),
        }
    }

    #[test]
    fn rows_count_per_order_and_tier_through_aliases() {
        let tiers = BTreeMap::from([tier(1, "Regular", &["Normal"]), tier(2, "Early Bird", &[])]);
        let o = orders(
            &sheet(&[
                ("regular", "1"),
                ("Normal", "1"),
                ("Early Bird", "1"),
                ("VIP", "2"),
                ("vip", "3"),
            ]),
            &tiers,
        );
        assert_eq!(
            o.orders,
            [
                OrderCount {
                    order: "1".into(),
                    tier: TierId(1),
                    count: 2
                },
                OrderCount {
                    order: "1".into(),
                    tier: TierId(2),
                    count: 1
                },
            ]
        );
        assert_eq!(
            o.unknown,
            [UnknownTier {
                name: "VIP".into(),
                rows: 2
            }]
        );
    }

    #[test]
    fn removals_are_found_per_order_even_when_the_tier_grows() {
        let tiers = BTreeMap::from([tier(1, "Regular", &[])]);
        let first = TicketImport {
            id: ImportId(1),
            filename: "a.ods".into(),
            sha256: "aa".into(),
            imported_at: OffsetDateTime::UNIX_EPOCH,
            date: OffsetDateTime::UNIX_EPOCH.date(),
            orders: orders(
                &sheet(&[("Regular", "1"), ("Regular", "1"), ("Regular", "2")]),
                &tiers,
            )
            .orders,
        };
        // Order 1 lost a ticket, order 3 bought two: the tier grows by one.
        let p = preview(
            &sheet(&[
                ("Regular", "1"),
                ("Regular", "2"),
                ("Regular", "3"),
                ("Regular", "3"),
            ]),
            &tiers,
            Some(&first),
            true,
        );
        assert_eq!(
            p.tiers,
            [TierChange {
                tier: TierId(1),
                before: 3,
                after: 4,
                added: 2,
                removed: 1
            }]
        );
        assert_eq!(
            p.removed,
            [OrderChange {
                order: "1".into(),
                tier: TierId(1),
                before: 2,
                after: 1
            }]
        );
        assert_eq!(p.previous, Some(ImportId(1)));
        assert!(!p.same_file);
    }

    #[test]
    fn names_and_aliases_are_unique_across_tiers() {
        let tiers = BTreeMap::from([tier(1, "Regular", &["Normal"])]);
        let draft = |name: &str, aliases: &[&str]| TierDraft {
            name: name.into(),
            price: Cents::new(1800).unwrap(),
            fee_bp: 0,
            quota: Some(10),
            aliases: aliases.iter().map(|a| (*a).into()).collect(),
        };
        let field = |r: Result<TierDraft, DomainError>| r.unwrap_err().field().map(str::to_owned);
        assert_eq!(
            field(clean_tier(draft("normal", &[]), None, &tiers)),
            Some("name".into())
        );
        assert_eq!(
            field(clean_tier(draft("Reduced", &["x", "NORMAL"]), None, &tiers)),
            Some("aliases[1]".into())
        );
        // Its own name and repeated aliases are dropped.
        let ok = clean_tier(
            draft(" Reduced ", &["reduced", "Erm.", "erm."]),
            None,
            &tiers,
        )
        .unwrap();
        assert_eq!(ok.name, "Reduced");
        assert_eq!(ok.aliases, ["Erm."]);
        // A tier may keep its own names.
        assert!(clean_tier(draft("Regular", &["Normal"]), Some(TierId(1)), &tiers).is_ok());
    }

    #[test]
    fn a_tier_rounds_its_fee_and_refund_on_each_ticket() {
        let tier = Tier {
            id: TierId(1),
            draft: TierDraft {
                name: "Early Bird".into(),
                price: Cents::new(2_500).unwrap(),
                fee_bp: 250,
                quota: None,
                aliases: Vec::new(),
            },
        };
        // 0,625 € rounds to 0,63 € on each ticket, not 1,875 € to 1,88 € on the three.
        let money = tier.money(3).unwrap();
        assert_eq!(money.gross.get(), 7_500);
        assert_eq!(money.fees.get(), 189);
        assert_eq!(money.net.get(), 7_311);
        assert_eq!(tier.refund(2_502, 3).unwrap().get(), 1_878);
        assert_eq!(tier.refund(2_502, 1).unwrap().get(), 626);
    }

    #[test]
    fn the_fee_must_stay_below_the_price() {
        let tiers = BTreeMap::new();
        let draft = |price: i64, fee_bp: u32| TierDraft {
            name: "T".into(),
            price: Cents::new(price).unwrap(),
            fee_bp,
            quota: None,
            aliases: Vec::new(),
        };
        assert!(clean_tier(draft(3200, 250), None, &tiers).is_ok());
        assert!(clean_tier(draft(0, 250), None, &tiers).is_ok());
        assert_eq!(
            clean_tier(draft(1, 5000), None, &tiers),
            Err(DomainError::FeeTooHigh)
        );
        assert_eq!(
            clean_tier(draft(3200, BP_FULL), None, &tiers),
            Err(DomainError::FeeTooHigh)
        );
    }
}
