//! The book's ticket commands (requirements §5.6) and the income they generate.

use std::collections::BTreeMap;

use time::{Date, OffsetDateTime};

use super::{Book, NextIds, clean};
use crate::error::DomainError;
use crate::model::{
    Account, AccountId, AccountKind, CancellationId, ImportId, Line, ManualSaleId, Origin, Role,
    TierId, Txn, TxnId, TxnKind,
};
use crate::money::BP_FULL;
use crate::sales::Profitability;
use crate::tickets::{
    self, Cancellation, CancellationDraft, CancellationEdit, CancellationSource, ImportDeletion,
    ImportPreview, ManualSale, ManualSaleDraft, RefundTerms, Resolution, TicketBatch, TicketImport,
    TicketSettings, Tickets, Tier, TierDraft, clean_tier, default_payer, most_ever,
};

/// The name of the income category of tickets.
const TICKETS: &str = "Tickets";
/// The name of the fee category when the first fee needs one.
const TICKET_FEES: &str = "Ticket fees";
/// The name of the refund category when the first refund needs one.
const TICKET_REFUNDS: &str = "Ticket refunds";

impl Book {
    // --- reading -------------------------------------------------------------------------

    pub fn tiers(&self) -> impl Iterator<Item = &Tier> {
        self.tiers.values()
    }

    pub fn tier(&self, id: TierId) -> Option<&Tier> {
        self.tiers.get(&id)
    }

    /// Oldest first.
    pub fn ticket_imports(&self) -> impl Iterator<Item = &TicketImport> {
        self.imports.values()
    }

    /// The import the ticket income comes from: the one with the latest date.
    pub fn latest_import(&self) -> Option<&TicketImport> {
        tickets::by_date(&self.imports).last().copied()
    }

    pub fn manual_sales(&self) -> impl Iterator<Item = &ManualSale> {
        self.manual_sales.values()
    }

    /// Cancellations and corrections, oldest first.
    pub fn cancellations(&self) -> impl Iterator<Item = &Cancellation> {
        self.cancellations.values()
    }

    /// Whether the event sells tickets, and where the sheet's money arrives.
    pub fn ticket_settings(&self) -> TicketSettings {
        self.ticket_settings
    }

    /// Ticket figures, always up to date.
    pub fn tickets(&self) -> &Tickets {
        &self.tickets
    }

    /// The projected result and break-even (R-TKT-9), once there is a budget and while the event
    /// is open.
    pub fn profitability(&self) -> Option<Profitability> {
        self.profitability
            .clone()
            .filter(|_| self.event.closed_at.is_none())
    }

    // --- tiers and settings --------------------------------------------------------------

    /// Adds a ticket tier (R-TKT-1, R-TKT-2, R-TKT-10).
    pub fn add_tier(&mut self, draft: TierDraft) -> Result<TierId, DomainError> {
        self.change(|book| {
            if !book.ticket_settings.enabled {
                return Err(DomainError::TicketsOff);
            }
            let draft = clean_tier(draft, None, &book.tiers)?;
            let id = TierId(book.next.tier);
            book.next.tier += 1;
            book.tiers.insert(id, Tier { id, draft });
            Ok(id)
        })
    }

    /// Changes a tier. Its ticket income follows, e.g. with a new price.
    pub fn update_tier(&mut self, id: TierId, draft: TierDraft) -> Result<(), DomainError> {
        self.change(|book| {
            if !book.tiers.contains_key(&id) {
                return Err(DomainError::UnknownTier);
            }
            let draft = clean_tier(draft, Some(id), &book.tiers)?;
            book.tiers.insert(id, Tier { id, draft });
            Ok(())
        })
    }

    /// Deletes a tier that no import and no manual sale uses.
    pub fn delete_tier(&mut self, id: TierId) -> Result<Tier, DomainError> {
        self.change(|book| {
            let imported = book
                .imports
                .values()
                .flat_map(|i| &i.orders)
                .any(|o| o.tier == id);
            let sold = book.manual_sales.values().any(|s| s.draft.tier == id);
            if imported || sold {
                return Err(DomainError::TierInUse);
            }
            book.tiers.remove(&id).ok_or(DomainError::UnknownTier)
        })
    }

    /// Switches tickets on or off for this event. Off is refused while there are tiers, and it takes
    /// the ticket categories along: one that something refers to stays as an ordinary category, so
    /// no figure changes. On, the sheet's money starts out arriving where reimbursements come from.
    pub fn set_tickets_enabled(&mut self, enabled: bool) -> Result<(), DomainError> {
        self.change(|book| {
            if enabled == book.ticket_settings.enabled {
                return Ok(());
            }
            if enabled {
                book.ticket_settings = TicketSettings {
                    enabled,
                    account: Some(book.event.default_reimbursement_account),
                };
                book.ticket_category(Role::Tickets, AccountKind::Income, TICKETS);
                return Ok(());
            }
            if !book.tiers.is_empty() {
                return Err(DomainError::TicketsInUse);
            }
            book.ticket_settings = TicketSettings {
                enabled,
                account: None,
            };
            let categories: Vec<AccountId> = book
                .accounts
                .values()
                .filter(|a| a.role.is_some())
                .map(|a| a.id)
                .collect();
            for id in categories {
                if book.category_in_use(id) {
                    if let Some(a) = book.accounts.get_mut(&id) {
                        a.role = None;
                    }
                } else {
                    book.accounts.remove(&id);
                }
            }
            Ok(())
        })
    }

    /// Sets the asset account the sheet's money arrives in. The ticket income moves with it. Once
    /// there are imports there must be one.
    pub fn set_ticket_account(&mut self, account: Option<AccountId>) -> Result<(), DomainError> {
        self.change(|book| {
            if !book.ticket_settings.enabled {
                return Err(DomainError::TicketsOff);
            }
            match account.map(|id| book.accounts.get(&id).map(|a| a.kind)) {
                None if !book.imports.is_empty() => return Err(DomainError::NoTicketAccount),
                None | Some(Some(AccountKind::Asset)) => {}
                Some(None) => {
                    return Err(DomainError::UnknownAccount {
                        field: "account".into(),
                    });
                }
                Some(Some(_)) => {
                    return Err(DomainError::WrongAccountKind {
                        field: "account".into(),
                    });
                }
            }
            book.ticket_settings.account = account;
            Ok(())
        })
    }

    // --- imports -------------------------------------------------------------------------

    /// What importing `batch` dated `date` would change, compared with the import before that
    /// date (R-TKT-5). Dated before the latest import, it does not become the current sheet.
    pub fn preview_import(&self, batch: &TicketBatch, date: Date) -> ImportPreview {
        let imports = tickets::by_date(&self.imports);
        let previous = imports.iter().rev().find(|i| i.date <= date).copied();
        let current = imports.last().is_none_or(|i| i.date <= date);
        let mut preview = tickets::preview(batch, &self.tiers, previous, current);
        let mut after = self.clone();
        if after
            .import_tickets(batch, date, OffsetDateTime::UNIX_EPOCH)
            .is_ok()
        {
            preview.receipts_lost = self.receipts_lost_in(&after);
        }
        preview
    }

    /// The receipts that are in this book but not in `after`, by file name.
    fn receipts_lost_in(&self, after: &Book) -> Vec<String> {
        self.attachments
            .values()
            .filter(|a| !after.attachments.contains_key(&a.id))
            .map(|a| a.filename.clone())
            .collect()
    }

    /// Deletes an import, such as a wrongly chosen sheet: everything goes back to the sheets that
    /// are left (R-TKT-5). Refused while a cancellation still needs its tickets.
    pub fn delete_import(&mut self, id: ImportId) -> Result<TicketImport, DomainError> {
        self.change(|book| {
            let import = book.imports.remove(&id).ok_or(DomainError::UnknownImport)?;
            let needed = book
                .cancellations
                .values()
                .any(|c| check_cancellation(c, book).is_err());
            if needed {
                return Err(DomainError::ImportInUse);
            }
            Ok(import)
        })
    }

    /// What [`Book::delete_import`] would change.
    pub fn preview_delete_import(&self, id: ImportId) -> Result<ImportDeletion, DomainError> {
        let mut after = self.clone();
        after.delete_import(id)?;
        Ok(ImportDeletion {
            sold_before: self.tickets.total.sold,
            sold_after: after.tickets.total.sold,
            net_before: self.tickets.total.net,
            net_after: after.tickets.total.net,
            conflicts_before: self.tickets.conflict_count(),
            conflicts_after: after.tickets.conflict_count(),
            receipts_lost: self.receipts_lost_in(&after),
        })
    }

    /// Imports a batch of tickets as a snapshot dated `date`, the current one unless an import is
    /// dated later (R-TKT-5, R-TKT-8).
    /// Tier names that match no tier block it (R-TKT-4). The ticket income is regenerated
    /// from it (R-TKT-6).
    pub fn import_tickets(
        &mut self,
        batch: &TicketBatch,
        date: Date,
        at: OffsetDateTime,
    ) -> Result<ImportId, DomainError> {
        self.change(|book| {
            if book.ticket_settings.account.is_none() {
                return Err(DomainError::NoTicketAccount);
            }
            // A batch comes from a plugin, so nothing about its shape is taken for granted.
            let source = batch.source.trim();
            if source.is_empty() {
                return Err(DomainError::EmptyName {
                    field: "source".into(),
                });
            }
            if batch.tickets.iter().any(|t| t.order.trim().is_empty()) {
                return Err(DomainError::EmptyName {
                    field: "order".into(),
                });
            }
            let orders = tickets::orders(batch, &book.tiers);
            if !orders.unknown.is_empty() {
                return Err(DomainError::UnknownTiers {
                    names: orders.unknown.into_iter().map(|u| u.name).collect(),
                });
            }
            let id = ImportId(book.next.import);
            book.next.import += 1;
            book.imports.insert(
                id,
                TicketImport {
                    id,
                    filename: source.to_owned(),
                    sha256: batch.sha256.clone(),
                    imported_at: at,
                    date,
                    orders: orders.orders,
                },
            );
            Ok(id)
        })
    }

    /// Changes the day an import stands for, e.g. for a file imported after the fact (R-TKT-8).
    /// This can change which import is current, and with it the income and the conflicts.
    pub fn set_import_date(&mut self, id: ImportId, date: Date) -> Result<(), DomainError> {
        self.change(|book| {
            let import = book
                .imports
                .get_mut(&id)
                .ok_or(DomainError::UnknownImport)?;
            import.date = date;
            Ok(())
        })
    }

    // --- manual sales --------------------------------------------------------------------

    /// Records tickets sold outside the sheet: income to the account or person who took the money
    /// (R-TKT-7).
    pub fn add_manual_sale(&mut self, draft: ManualSaleDraft) -> Result<ManualSaleId, DomainError> {
        self.change(|book| {
            let draft = clean_sale(draft, &book.tiers, &book.accounts)?;
            let id = ManualSaleId(book.next.manual_sale);
            book.next.manual_sale += 1;
            book.manual_sales.insert(id, ManualSale { id, draft });
            Ok(id)
        })
    }

    pub fn update_manual_sale(
        &mut self,
        id: ManualSaleId,
        draft: ManualSaleDraft,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let current = book
                .manual_sales
                .get(&id)
                .ok_or(DomainError::UnknownManualSale)?;
            let draft = clean_sale(draft, &book.tiers, &book.accounts)?;
            let cancelled: u32 = book
                .cancellations
                .values()
                .filter(|c| c.sale == Some(id))
                .map(|c| c.count)
                .sum();
            if cancelled > 0 && draft.tier != current.draft.tier {
                return Err(DomainError::SaleHasCancellations);
            }
            if draft.count < cancelled {
                return Err(DomainError::TooManyTickets);
            }
            book.manual_sales.insert(id, ManualSale { id, draft });
            Ok(())
        })
    }

    pub fn delete_manual_sale(&mut self, id: ManualSaleId) -> Result<ManualSale, DomainError> {
        self.change(|book| {
            if book.cancellations.values().any(|c| c.sale == Some(id)) {
                return Err(DomainError::SaleHasCancellations);
            }
            book.manual_sales
                .remove(&id)
                .ok_or(DomainError::UnknownManualSale)
        })
    }

    // --- cancellations -------------------------------------------------------------------

    /// Records a cancellation without a change in the sheet, on a sheet order or a manual sale
    /// (R-TKT-14). Its refund counts from now on; its tickets stay sold.
    pub fn add_cancellation(
        &mut self,
        draft: CancellationDraft,
    ) -> Result<CancellationId, DomainError> {
        self.change(|book| {
            let (tier, order, sale) = match draft.source {
                CancellationSource::Order { order, tier } => {
                    let order = order.trim().to_owned();
                    if most_ever(&book.imports, &order, tier) == 0 {
                        return Err(DomainError::UnknownOrder);
                    }
                    (tier, Some(order), None)
                }
                CancellationSource::Sale { sale } => {
                    let tier = book
                        .manual_sales
                        .get(&sale)
                        .ok_or(DomainError::UnknownManualSale)?
                        .draft
                        .tier;
                    (tier, None, Some(sale))
                }
            };
            let id = CancellationId(book.next.cancellation);
            book.next.cancellation += 1;
            let c = Cancellation {
                id,
                tier,
                count: draft.count,
                order,
                sale,
                gone: 0,
                note: clean(draft.note),
                terms: Some(draft.terms),
            };
            check_cancellation(&c, book)?;
            book.cancellations.insert(id, c);
            Ok(id)
        })
    }

    /// Changes the count, refund, date, payer or note of a cancellation.
    pub fn update_cancellation(
        &mut self,
        id: CancellationId,
        edit: CancellationEdit,
    ) -> Result<(), DomainError> {
        self.change(|book| {
            let current = book
                .cancellations
                .get(&id)
                .ok_or(DomainError::UnknownCancellation)?;
            if current.terms.is_none() {
                return Err(DomainError::CorrectionNotEditable);
            }
            let c = Cancellation {
                count: edit.count,
                note: clean(edit.note),
                terms: Some(edit.terms),
                ..current.clone()
            };
            check_cancellation(&c, book)?;
            book.cancellations.insert(id, c);
            Ok(())
        })
    }

    /// Deletes a cancellation or correction. Tickets it resolved are a conflict again.
    pub fn delete_cancellation(&mut self, id: CancellationId) -> Result<Cancellation, DomainError> {
        self.change(|book| {
            book.cancellations
                .remove(&id)
                .ok_or(DomainError::UnknownCancellation)
        })
    }

    /// Resolves tickets of a conflict, all of them or some (R-TKT-13).
    pub fn resolve_conflict(
        &mut self,
        order: &str,
        tier: TierId,
        resolution: Resolution,
    ) -> Result<CancellationId, DomainError> {
        self.change(|book| {
            let count = match &resolution {
                Resolution::Refund { count, .. }
                | Resolution::Match { count, .. }
                | Resolution::Correction { count, .. } => *count,
            };
            if count == 0 {
                return Err(DomainError::NonPositiveAmount {
                    field: "count".into(),
                });
            }
            let open = tickets::reckon(&tickets::by_date(&book.imports), &book.cancellations, None)
                .unresolved
                .get(&(order.to_owned(), tier))
                .map_or(0, |(count, _)| *count);
            if count > open {
                return Err(DomainError::TooManyTickets);
            }

            if let Resolution::Match { cancellation, .. } = resolution {
                let c = book
                    .cancellations
                    .get_mut(&cancellation)
                    .filter(|c| {
                        c.tier == tier && c.order.as_deref() == Some(order) && c.terms.is_some()
                    })
                    .ok_or(DomainError::UnknownCancellation)?;
                if c.gone + count > c.count {
                    return Err(DomainError::TooManyTickets);
                }
                c.gone += count;
                return Ok(cancellation);
            }

            let (terms, note) = match resolution {
                Resolution::Refund { terms, note, .. } => (Some(terms), note),
                Resolution::Correction { note, .. } => (None, note),
                Resolution::Match { .. } => unreachable!("handled above"),
            };
            let id = CancellationId(book.next.cancellation);
            book.next.cancellation += 1;
            let c = Cancellation {
                id,
                tier,
                count,
                order: Some(order.to_owned()),
                sale: None,
                gone: count,
                note: clean(note),
                terms,
            };
            check_cancellation(&c, book)?;
            book.cancellations.insert(id, c);
            Ok(id)
        })
    }

    // --- generated income ----------------------------------------------------------------

    /// Makes the ticket income match the tickets (R-TKT-6, R-TKT-7, R-TKT-15): one income per tier
    /// of the latest import and its cancelled tickets, to the ticket account, net of the tier's
    /// fee; one per manual sale, to whoever took the money; and one negative income per
    /// refund, from whoever pays it. Existing ones keep their ids; ones no longer needed go.
    pub(super) fn sync_tickets(&mut self) -> Result<(), DomainError> {
        let existing: BTreeMap<Origin, TxnId> = self
            .txns
            .values()
            .filter(|t| t.origin.is_generated())
            .map(|t| (t.origin, t.id))
            .collect();
        if existing.is_empty() && self.imports.is_empty() && self.manual_sales.is_empty() {
            return Ok(());
        }
        let income = self.ticket_category(Role::Tickets, AccountKind::Income, TICKETS);
        let ticket_account = self.ticket_settings.account;
        let line = |account, amount| Line {
            account,
            amount,
            budget_line: None,
            cost_centre: None,
        };

        let mut wanted: BTreeMap<Origin, (Date, String, Option<String>, Vec<Line>)> =
            BTreeMap::new();
        if let Some(latest) = self.latest_import() {
            let date = latest.date;
            let account = ticket_account.ok_or(DomainError::NoTicketAccount)?;
            let mut counts = latest.counts();
            let reckoning =
                tickets::reckon(&tickets::by_date(&self.imports), &self.cancellations, None);
            for (tier, cancelled) in reckoning.cancelled {
                *counts.entry(tier).or_insert(0) += cancelled;
            }
            for (tier, count) in counts {
                let t = self.tiers.get(&tier).ok_or(DomainError::UnknownTier)?;
                let money = t.money(count)?;
                if money.gross.is_zero() {
                    continue;
                }
                let description = format!("{count} × {}", t.draft.name);
                let mut lines = vec![line(income, -money.gross)];
                if money.fees.is_positive() {
                    lines.push(line(self.ticket_fees_account(), money.fees));
                }
                lines.push(line(account, money.net));
                wanted.insert(
                    Origin::TicketImport { tier },
                    (date, description, None, lines),
                );
            }
        }
        for s in self.manual_sales.values() {
            let t = self
                .tiers
                .get(&s.draft.tier)
                .ok_or(DomainError::UnknownTier)?;
            let gross = t.money(s.draft.count)?.gross;
            if gross.is_zero() {
                continue;
            }
            wanted.insert(
                Origin::TicketSale { sale: s.id },
                (
                    s.draft.date,
                    format!("{} × {}", s.draft.count, t.draft.name),
                    s.draft.reference.clone(),
                    vec![line(income, -gross), line(s.draft.destination, gross)],
                ),
            );
        }

        let refunds: Vec<Cancellation> = self.cancellations.values().cloned().collect();
        for c in refunds {
            let Some(terms) = c.terms else { continue };
            let t = self.tiers.get(&c.tier).ok_or(DomainError::UnknownTier)?;
            let refund = t.refund(terms.refund_bp, c.count)?;
            if refund.is_zero() {
                continue;
            }
            let description = format!("Refund {} × {}", c.count, t.draft.name);
            let payer = terms
                .payer
                .or(default_payer(&c, &self.manual_sales, ticket_account))
                .ok_or(DomainError::UnknownManualSale)?;
            wanted.insert(
                Origin::TicketRefund { cancellation: c.id },
                (
                    terms.date,
                    description,
                    c.note,
                    vec![
                        line(self.ticket_refunds_account(), refund),
                        line(payer, -refund),
                    ],
                ),
            );
        }

        for (origin, id) in &existing {
            if !wanted.contains_key(origin) {
                self.remove_txn(*id);
            }
        }
        for (origin, (date, description, notes, lines)) in wanted {
            let id = match existing.get(&origin) {
                Some(id) => *id,
                None => {
                    let id = TxnId(self.next.txn);
                    self.next.txn += 1;
                    id
                }
            };
            self.txns.insert(
                id,
                Txn {
                    id,
                    kind: match origin {
                        Origin::TicketRefund { .. } => TxnKind::Refund,
                        _ => TxnKind::Income,
                    },
                    date,
                    description,
                    counterparty: None,
                    notes,
                    lines,
                    origin,
                },
            );
        }
        Ok(())
    }

    /// The generated transactions by where they come from.
    pub(super) fn generated_txns(&self) -> BTreeMap<Origin, TxnId> {
        self.txns
            .values()
            .filter(|t| t.origin.is_generated())
            .map(|t| (t.origin, t.id))
            .collect()
    }

    fn ticket_fees_account(&mut self) -> AccountId {
        self.ticket_category(Role::TicketFees, AccountKind::Expense, TICKET_FEES)
    }

    fn ticket_refunds_account(&mut self) -> AccountId {
        self.ticket_category(Role::TicketRefunds, AccountKind::Income, TICKET_REFUNDS)
    }

    /// A generated category: the one with the role, else an unassigned one with the name, else
    /// a new one.
    pub(super) fn ticket_category(
        &mut self,
        role: Role,
        kind: AccountKind,
        name: &str,
    ) -> AccountId {
        if let Some(a) = self.account_by_role(role) {
            return a.id;
        }
        let named = self
            .account_by_name(kind, name)
            .filter(|a| a.role.is_none())
            .map(|a| a.id);
        match named.and_then(|id| self.accounts.get_mut(&id)) {
            Some(a) => {
                a.role = Some(role);
                a.id
            }
            None => self.insert_account(kind, name, Some(role)),
        }
    }
}

/// Checks a cancellation against the rest of the book: a known tier and source, a refund of at
/// most 100 % from an asset or a person, a trimmed note, and no more tickets than there are.
fn check_cancellation(c: &Cancellation, book: &Book) -> Result<(), DomainError> {
    check_cancellation_parts(
        c,
        &book.tiers,
        &book.imports,
        &book.manual_sales,
        &book.cancellations,
        &book.accounts,
    )
}

fn check_cancellation_parts(
    c: &Cancellation,
    tiers: &BTreeMap<TierId, Tier>,
    imports: &BTreeMap<ImportId, TicketImport>,
    sales: &BTreeMap<ManualSaleId, ManualSale>,
    others: &BTreeMap<CancellationId, Cancellation>,
    accounts: &BTreeMap<AccountId, Account>,
) -> Result<(), DomainError> {
    if !tiers.contains_key(&c.tier) {
        return Err(DomainError::UnknownTier);
    }
    if c.count == 0 {
        return Err(DomainError::NonPositiveAmount {
            field: "count".into(),
        });
    }
    if let Some(RefundTerms {
        refund_bp, payer, ..
    }) = c.terms
    {
        if refund_bp > BP_FULL {
            return Err(DomainError::RefundTooHigh);
        }
        if let Some(payer) = payer {
            match accounts.get(&payer).map(|a| a.kind) {
                Some(AccountKind::Asset | AccountKind::Person) => {}
                None => {
                    return Err(DomainError::UnknownAccount {
                        field: "payer".into(),
                    });
                }
                Some(_) => {
                    return Err(DomainError::WrongAccountKind {
                        field: "payer".into(),
                    });
                }
            }
        }
    }
    let same_source = others
        .values()
        .filter(|o| o.id != c.id && o.sale == c.sale && o.order == c.order && o.tier == c.tier);
    let taken: u32 = same_source.map(|o| o.count).sum();
    let available = match (&c.order, c.sale) {
        (Some(order), None) => most_ever(imports, order, c.tier),
        (None, Some(sale)) => {
            let s = sales.get(&sale).ok_or(DomainError::UnknownManualSale)?;
            if s.draft.tier != c.tier {
                return Err(DomainError::UnknownManualSale);
            }
            s.draft.count
        }
        _ => return Err(DomainError::UnknownOrder),
    };
    if available == 0 {
        return Err(DomainError::UnknownOrder);
    }
    if taken + c.count > available || c.gone > c.count {
        return Err(DomainError::TooManyTickets);
    }
    Ok(())
}

/// A tier that exists, a positive count, an asset or a person, and a trimmed reference.
fn clean_sale(
    d: ManualSaleDraft,
    tiers: &BTreeMap<TierId, Tier>,
    accounts: &BTreeMap<AccountId, Account>,
) -> Result<ManualSaleDraft, DomainError> {
    if !tiers.contains_key(&d.tier) {
        return Err(DomainError::UnknownTier);
    }
    if d.count == 0 {
        return Err(DomainError::NonPositiveAmount {
            field: "count".into(),
        });
    }
    match accounts.get(&d.destination).map(|a| a.kind) {
        Some(AccountKind::Asset | AccountKind::Person) => {}
        None => {
            return Err(DomainError::UnknownAccount {
                field: "destination".into(),
            });
        }
        Some(_) => {
            return Err(DomainError::WrongAccountKind {
                field: "destination".into(),
            });
        }
    }
    Ok(ManualSaleDraft {
        reference: clean(d.reference),
        ..d
    })
}

type TicketMaps = (
    BTreeMap<TierId, Tier>,
    BTreeMap<ImportId, TicketImport>,
    BTreeMap<ManualSaleId, ManualSale>,
    BTreeMap<CancellationId, Cancellation>,
);

/// Checks stored tiers, imports, manual sales and cancellations the way the commands would have
/// made them.
pub(super) fn check_parts(
    tiers: Vec<Tier>,
    imports: Vec<TicketImport>,
    sales: Vec<ManualSale>,
    cancellations: Vec<Cancellation>,
    next: &NextIds,
    accounts: &BTreeMap<AccountId, Account>,
) -> Result<TicketMaps, DomainError> {
    let bad = |reason: &str| DomainError::Inconsistent {
        reason: reason.to_owned(),
    };

    let mut tier_map = BTreeMap::new();
    for t in tiers {
        if t.id.0 >= next.tier
            || clean_tier(t.draft.clone(), Some(t.id), &tier_map).as_ref() != Ok(&t.draft)
        {
            return Err(bad("ticket tier with an invalid name, price, fee or alias"));
        }
        tier_map.insert(t.id, t);
    }

    let mut import_map = BTreeMap::new();
    for i in imports {
        let sorted = i
            .orders
            .windows(2)
            .all(|w| (&w[0].order, w[0].tier) < (&w[1].order, w[1].tier));
        let valid = i
            .orders
            .iter()
            .all(|o| o.count > 0 && tier_map.contains_key(&o.tier));
        if i.id.0 >= next.import || !sorted || !valid {
            return Err(bad("ticket import with unknown tiers or repeated orders"));
        }
        import_map.insert(i.id, i);
    }

    let mut sale_map = BTreeMap::new();
    for s in sales {
        if s.id.0 >= next.manual_sale
            || clean_sale(s.draft.clone(), &tier_map, accounts).as_ref() != Ok(&s.draft)
        {
            return Err(bad("manual sale with an unknown tier or destination"));
        }
        sale_map.insert(s.id, s);
    }

    let mut cancellation_map = BTreeMap::new();
    for c in cancellations {
        let correction_ok = c.terms.is_some() || (c.order.is_some() && c.gone == c.count);
        if c.id.0 >= next.cancellation
            || !correction_ok
            || c.note != clean(c.note.clone())
            || check_cancellation_parts(
                &c,
                &tier_map,
                &import_map,
                &sale_map,
                &cancellation_map,
                accounts,
            )
            .is_err()
        {
            return Err(bad("ticket cancellation that does not fit its tickets"));
        }
        cancellation_map.insert(c.id, c);
    }
    Ok((tier_map, import_map, sale_map, cancellation_map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountDraft, AssetType, EventDetails};

    #[test]
    fn a_fee_category_with_the_name_is_reused() {
        let details = EventDetails {
            currency: Default::default(),
            name: "E".into(),
            starts_on: None,
            ends_on: None,
            location: None,
            notes: None,
        };
        let accounts = [AccountDraft {
            name: "Bank".into(),
            asset_type: AssetType::Bank,
        }];
        let mut book = Book::new(details, &accounts).unwrap();
        let own = book
            .add_account(AccountKind::Expense, "ticket fees")
            .unwrap();
        assert_eq!(book.ticket_fees_account(), own);
        assert_eq!(book.account(own).unwrap().role, Some(Role::TicketFees));
        assert_eq!(book.ticket_fees_account(), own);
    }
}
