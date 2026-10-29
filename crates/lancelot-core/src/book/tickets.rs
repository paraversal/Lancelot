//! The book's ticket commands (requirements §5.6) and the income they generate.

use std::collections::BTreeMap;

use time::{Date, OffsetDateTime};

use super::{Book, NextIds, clean};
use crate::error::DomainError;
use crate::model::{
    Account, AccountId, AccountKind, ImportId, Line, ManualSaleId, Origin, Role, TierId, Txn,
    TxnId, TxnKind,
};
use crate::sheet::Sheet;
use crate::tickets::{
    self, ImportPreview, ManualSale, ManualSaleDraft, TicketColumns, TicketImport, Tickets, Tier,
    TierDraft, clean_tier,
};

/// The name of the fee category when the first fee needs one.
const TICKET_FEES: &str = "Ticket fees";

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

    /// The import the ticket income comes from.
    pub fn latest_import(&self) -> Option<&TicketImport> {
        self.imports.values().next_back()
    }

    pub fn manual_sales(&self) -> impl Iterator<Item = &ManualSale> {
        self.manual_sales.values()
    }

    /// Ticket figures, always up to date.
    pub fn tickets(&self) -> &Tickets {
        &self.tickets
    }

    // --- tiers and settings --------------------------------------------------------------

    /// Adds a ticket tier (R-TKT-1, R-TKT-2, R-TKT-10).
    pub fn add_tier(&mut self, draft: TierDraft) -> Result<TierId, DomainError> {
        self.change(|book| {
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

    /// Sets the columns the ticket sheet is read from (R-TKT-3).
    pub fn set_ticket_columns(&mut self, columns: TicketColumns) -> Result<(), DomainError> {
        self.change(|book| {
            let name = |text: &str, field: &str| {
                let text = text.trim();
                if text.is_empty() {
                    return Err(DomainError::EmptyName {
                        field: field.into(),
                    });
                }
                Ok(text.to_owned())
            };
            let tier = name(&columns.tier, "tier")?;
            let order = name(&columns.order, "order")?;
            let sale_date = clean(columns.sale_date);
            let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
            if same(&tier, &order) {
                return Err(DomainError::DuplicateName {
                    field: "order".into(),
                });
            }
            if sale_date
                .as_deref()
                .is_some_and(|d| same(d, &tier) || same(d, &order))
            {
                return Err(DomainError::DuplicateName {
                    field: "saleDate".into(),
                });
            }
            book.event.ticket_columns = TicketColumns {
                tier,
                order,
                sale_date,
            };
            Ok(())
        })
    }

    // --- imports -------------------------------------------------------------------------

    /// What importing `sheet` would change, compared with the latest import (R-TKT-5).
    pub fn preview_import(&self, sheet: &Sheet) -> ImportPreview {
        tickets::preview(sheet, &self.tiers, self.latest_import())
    }

    /// Imports a ticket sheet as the new current snapshot, dated `date` (R-TKT-5, R-TKT-8).
    /// Tier names that match no tier block it (R-TKT-4). The ticket income is regenerated
    /// from it (R-TKT-6).
    pub fn import_tickets(
        &mut self,
        sheet: &Sheet,
        date: Date,
        at: OffsetDateTime,
    ) -> Result<ImportId, DomainError> {
        self.change(|book| {
            let orders = tickets::orders(sheet, &book.tiers);
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
                    filename: sheet.filename.trim().to_owned(),
                    sha256: sheet.sha256.clone(),
                    imported_at: at,
                    date,
                    orders: orders.orders,
                },
            );
            Ok(id)
        })
    }

    /// Changes the day an import stands for, e.g. for a file imported after the fact (R-TKT-8).
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

    /// Records tickets sold outside the sheet: income to the person who took the money
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
            if !book.manual_sales.contains_key(&id) {
                return Err(DomainError::UnknownManualSale);
            }
            let draft = clean_sale(draft, &book.tiers, &book.accounts)?;
            book.manual_sales.insert(id, ManualSale { id, draft });
            Ok(())
        })
    }

    pub fn delete_manual_sale(&mut self, id: ManualSaleId) -> Result<ManualSale, DomainError> {
        self.change(|book| {
            book.manual_sales
                .remove(&id)
                .ok_or(DomainError::UnknownManualSale)
        })
    }

    // --- generated income ----------------------------------------------------------------

    /// Makes the ticket income match the tickets (R-TKT-6, R-TKT-7): one income per tier of the
    /// latest import, to the Verein account, net of the tier's fee; and one per manual sale, to
    /// the person who took the money. Existing ones keep their ids; ones no longer needed go.
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
        let role = |book: &Book, role| book.account_by_role(role).map(|a| a.id);
        let (Some(income), Some(verein)) = (role(self, Role::Tickets), role(self, Role::Verein))
        else {
            return Err(DomainError::Inconsistent {
                reason: "the Tickets or Verein account is missing".into(),
            });
        };
        let line = |account, amount| Line {
            account,
            amount,
            memo: None,
            budget_line: None,
        };

        let mut wanted: BTreeMap<Origin, (Date, String, Option<String>, Vec<Line>)> =
            BTreeMap::new();
        if let Some(latest) = self.latest_import() {
            let date = latest.date;
            for (tier, count) in latest.counts() {
                let t = self.tiers.get(&tier).ok_or(DomainError::UnknownTier)?;
                let gross = t.draft.price.checked_mul(i64::from(count))?;
                if gross.is_zero() {
                    continue;
                }
                let fee = t.fee()?.checked_mul(i64::from(count))?;
                let description = format!("{count} × {}", t.draft.name);
                let mut lines = vec![line(income, -gross)];
                if fee.is_positive() {
                    lines.push(line(self.ticket_fees_account(), fee));
                }
                lines.push(line(verein, gross.checked_sub(fee)?));
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
            let gross = t.draft.price.checked_mul(i64::from(s.draft.count))?;
            if gross.is_zero() {
                continue;
            }
            wanted.insert(
                Origin::TicketSale { sale: s.id },
                (
                    s.draft.date,
                    format!("{} × {}", s.draft.count, t.draft.name),
                    s.draft.reference.clone(),
                    vec![line(income, -gross), line(s.draft.person, gross)],
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
                    kind: TxnKind::Income,
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

    /// The generated income of each manual sale.
    pub(super) fn sale_txns(&self) -> BTreeMap<ManualSaleId, TxnId> {
        self.txns
            .values()
            .filter_map(|t| match t.origin {
                Origin::TicketSale { sale } => Some((sale, t.id)),
                _ => None,
            })
            .collect()
    }

    /// The fee category: the one with the role, else an unassigned `Ticket fees` category,
    /// else a new one.
    fn ticket_fees_account(&mut self) -> AccountId {
        if let Some(a) = self.account_by_role(Role::TicketFees) {
            return a.id;
        }
        let named = self
            .account_by_name(AccountKind::Expense, TICKET_FEES)
            .filter(|a| a.role.is_none())
            .map(|a| a.id);
        match named.and_then(|id| self.accounts.get_mut(&id)) {
            Some(a) => {
                a.role = Some(Role::TicketFees);
                a.id
            }
            None => self.insert_account(AccountKind::Expense, TICKET_FEES, Some(Role::TicketFees)),
        }
    }
}

/// A tier that exists, a positive count, a person and a trimmed reference.
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
    match accounts.get(&d.person).map(|a| a.kind) {
        Some(AccountKind::Person) => {}
        None => return Err(DomainError::UnknownPerson),
        Some(_) => {
            return Err(DomainError::WrongAccountKind {
                field: "person".into(),
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
);

/// Checks stored tiers, imports and manual sales the way the commands would have made them.
pub(super) fn check_parts(
    tiers: Vec<Tier>,
    imports: Vec<TicketImport>,
    sales: Vec<ManualSale>,
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
            return Err(bad("manual sale with an unknown tier or person"));
        }
        sale_map.insert(s.id, s);
    }
    Ok((tier_map, import_map, sale_map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EventDetails;

    #[test]
    fn a_fee_category_with_the_name_is_reused() {
        let mut book = Book::new(EventDetails {
            name: "E".into(),
            starts_on: None,
            ends_on: None,
            location: None,
            notes: None,
        })
        .unwrap();
        let own = book
            .add_account(AccountKind::Expense, "ticket fees")
            .unwrap();
        assert_eq!(book.ticket_fees_account(), own);
        assert_eq!(book.account(own).unwrap().role, Some(Role::TicketFees));
        assert_eq!(book.ticket_fees_account(), own);
    }
}
