//! Sales over time, imports in date order, places against the quota and break-even (R-TKT-8,
//! R-TKT-9).

mod common;

use common::*;
use lancelot_core::*;
use proptest::prelude::*;

fn tier(name: &str, price: &str, fee_bp: u32, quota: Option<u32>) -> TierDraft {
    TierDraft {
        name: name.into(),
        price: eur(price),
        fee_bp,
        quota,
        aliases: Vec::new(),
    }
}

/// Early Bird at 25 € and Regular at 32 € with a 2,5 % fee and 10 places.
fn with_tiers() -> (Book, TierId, TierId) {
    let mut book = new_book();
    let early = book.add_tier(tier("Early Bird", "25", 0, None)).unwrap();
    let regular = book.add_tier(tier("Regular", "32", 250, Some(10))).unwrap();
    (book, early, regular)
}

fn sheet(book: &Book, name: &str, rows: &[(&str, &str)]) -> Sheet {
    let mut text = String::from("Order ID,Ticket\n");
    for (order, tier) in rows {
        text.push_str(&format!("{order},{tier}\n"));
    }
    read_sheet(name, text.as_bytes(), &book.event().ticket_columns).unwrap()
}

fn import(book: &mut Book, day: u8, rows: &[(&str, &str)]) -> ImportId {
    let s = sheet(book, &format!("tickets-{day}.csv"), rows);
    book.import_tickets(&s, june(day), now()).unwrap()
}

fn terms(refund_bp: u32, day: u8) -> RefundTerms {
    RefundTerms {
        refund_bp,
        date: june(day),
        payer: None,
    }
}

fn refund(count: u32, refund_bp: u32, day: u8) -> Resolution {
    Resolution::Refund {
        count,
        terms: terms(refund_bp, day),
        note: None,
    }
}

fn sale(book: &mut Book, tier: TierId, count: u32, day: u8) -> ManualSaleId {
    let person = person(book, &format!("Seller {day}"));
    book.add_manual_sale(ManualSaleDraft {
        tier,
        count,
        person,
        date: june(day),
        reference: None,
    })
    .unwrap()
}

fn figures(book: &Book, id: TierId) -> TicketTotals {
    book.tickets()
        .tiers
        .iter()
        .find(|t| t.tier.id == id)
        .unwrap()
        .figures
}

// --- imports in date order -------------------------------------------------------------------

#[test]
fn a_file_imported_after_the_fact_takes_its_place_in_the_past() {
    let (mut book, _, regular) = with_tiers();
    let june_9 = import(
        &mut book,
        9,
        &[("1", "Regular"), ("2", "Regular"), ("3", "Regular")],
    );

    // The preview counts from the import before its date, and says it won't be current.
    let old = sheet(&book, "old.csv", &[("1", "Regular")]);
    let preview = book.preview_import(&old, june(2));
    assert_eq!(preview.previous, None);
    assert!(!preview.current);
    assert!(preview.removed.is_empty());
    let june_2 = book.import_tickets(&old, june(2), now()).unwrap();

    // The later file stays current: same income, no conflicts.
    assert_eq!(book.latest_import().unwrap().id, june_9);
    assert_eq!(figures(&book, regular).sold, 3);
    assert!(book.tickets().conflicts.is_empty());
    assert_eq!(
        book.tickets()
            .imports
            .iter()
            .map(|i| i.import.id)
            .collect::<Vec<_>>(),
        [june_2, june_9]
    );
    let sold: Vec<u32> = book.tickets().sales.iter().map(|p| p.total.sold).collect();
    assert_eq!(sold, [1, 3]);

    // A file dated last is current, and counts from the one before it.
    let new = sheet(&book, "new.csv", &[("1", "Regular"), ("2", "Regular")]);
    let preview = book.preview_import(&new, june(16));
    assert_eq!(preview.previous, Some(june_9));
    assert!(preview.current);
    assert_eq!(preview.removed.len(), 1);
}

#[test]
fn changing_a_date_can_change_the_current_import() {
    let (mut book, _, regular) = with_tiers();
    let first = import(&mut book, 2, &[("1", "Regular")]);
    let second = import(&mut book, 9, &[("1", "Regular"), ("2", "Regular")]);
    assert_eq!(figures(&book, regular).sold, 2);

    // Order 2 now leaves the sheet in the later import: a conflict, and not sold until resolved.
    book.set_import_date(first, june(12)).unwrap();
    assert_eq!(book.latest_import().unwrap().id, first);
    assert_eq!(figures(&book, regular).sold, 1);
    assert_eq!(figures(&book, regular).unresolved, 1);

    book.set_import_date(second, june(13)).unwrap();
    assert_eq!(book.latest_import().unwrap().id, second);
    assert_eq!(figures(&book, regular).sold, 2);
    assert_eq!(figures(&book, regular).unresolved, 0);
}

#[test]
fn imports_on_one_day_count_in_the_order_they_came() {
    let (mut book, _, regular) = with_tiers();
    import(&mut book, 2, &[("1", "Regular")]);
    let s = sheet(&book, "same-day.csv", &[("1", "Regular"), ("2", "Regular")]);
    let preview = book.preview_import(&s, june(2));
    assert!(preview.current);
    book.import_tickets(&s, june(2), now()).unwrap();
    assert_eq!(figures(&book, regular).sold, 2);
}

// --- places against the quota ----------------------------------------------------------------

#[test]
fn a_cancelled_ticket_frees_its_place_and_keeps_its_income() {
    let (mut book, _, regular) = with_tiers();
    import(
        &mut book,
        1,
        &[("1", "Regular"), ("1", "Regular"), ("2", "Regular")],
    );
    import(&mut book, 2, &[("1", "Regular"), ("2", "Regular")]);
    book.resolve_conflict("1", regular, refund(1, 5000, 3))
        .unwrap();

    // Half the price stays income, the ticket stays sold, but the place is free again.
    let t = figures(&book, regular);
    assert_eq!(t.sold, 3);
    assert_eq!(t.places, 2);
    assert_eq!(t.gross, eur("96"));
    assert_eq!(t.refunds, eur("16"));
}

#[test]
fn a_refund_of_nothing_frees_the_place_too() {
    let (mut book, _, regular) = with_tiers();
    import(&mut book, 1, &[("1", "Regular"), ("2", "Regular")]);
    import(&mut book, 2, &[("2", "Regular")]);
    book.resolve_conflict("1", regular, refund(1, 0, 3))
        .unwrap();
    let t = figures(&book, regular);
    assert_eq!((t.sold, t.places, t.refunds), (2, 1, Cents::ZERO));
}

#[test]
fn a_correction_was_never_sold_and_takes_no_place() {
    let (mut book, _, regular) = with_tiers();
    import(&mut book, 1, &[("1", "Regular"), ("2", "Regular")]);
    import(&mut book, 2, &[("2", "Regular")]);
    book.resolve_conflict(
        "1",
        regular,
        Resolution::Correction {
            count: 1,
            note: None,
        },
    )
    .unwrap();
    let t = figures(&book, regular);
    assert_eq!((t.sold, t.places), (1, 1));
}

#[test]
fn cancelling_a_ticket_still_in_the_sheet_or_a_manual_sale_frees_its_place() {
    let (mut book, _, regular) = with_tiers();
    import(&mut book, 1, &[("1", "Regular"), ("2", "Regular")]);
    let door = sale(&mut book, regular, 3, 2);
    book.add_cancellation(CancellationDraft {
        source: CancellationSource::Order {
            order: "1".into(),
            tier: regular,
        },
        count: 1,
        terms: terms(5000, 3),
        note: None,
    })
    .unwrap();
    book.add_cancellation(CancellationDraft {
        source: CancellationSource::Sale { sale: door },
        count: 2,
        terms: terms(10_000, 3),
        note: None,
    })
    .unwrap();
    let t = figures(&book, regular);
    assert_eq!((t.sold, t.places), (5, 2));
}

#[test]
fn a_cancellation_of_tickets_gone_without_a_resolution_frees_nothing_twice() {
    let (mut book, _, regular) = with_tiers();
    import(&mut book, 1, &[("1", "Regular"), ("2", "Regular")]);
    book.add_cancellation(CancellationDraft {
        source: CancellationSource::Order {
            order: "1".into(),
            tier: regular,
        },
        count: 1,
        terms: terms(5000, 1),
        note: None,
    })
    .unwrap();
    // Order 1 leaves: until it's confirmed the ticket isn't sold, so it has no place to free.
    import(&mut book, 2, &[("2", "Regular")]);
    let t = figures(&book, regular);
    assert_eq!((t.sold, t.unresolved, t.places), (1, 1, 1));
}

// --- sales over time -------------------------------------------------------------------------

#[test]
fn the_curve_follows_the_imports_and_its_tickets_in_time() {
    let (mut book, early, regular) = with_tiers();
    let first = import(&mut book, 1, &[("1", "Regular"), ("2", "Early Bird")]);
    sale(&mut book, early, 2, 3);
    let second = import(
        &mut book,
        5,
        &[("2", "Early Bird"), ("3", "Regular"), ("4", "Regular")],
    );
    // Order 1 left in the second import; the refund came later.
    book.resolve_conflict("1", regular, refund(1, 10_000, 7))
        .unwrap();
    sale(&mut book, early, 1, 9);

    let sales = &book.tickets().sales;
    let points: Vec<_> = sales
        .iter()
        .map(|p| (p.date, p.import, p.total.sold, p.total.places))
        .collect();
    assert_eq!(
        points,
        [
            (june(1), Some(first), 2, 2),
            // Sold counts the cancelled ticket from the import it left; its place is held until
            // the day it was cancelled.
            (june(5), Some(second), 6, 6),
            (june(9), None, 7, 6),
        ]
    );

    // Money at each point: the refund only from its date on.
    assert_eq!(sales[1].total.refunds, Cents::ZERO);
    assert_eq!(sales[2].total.refunds, eur("32"));

    // Per tier, in creation order.
    let per_tier: Vec<u32> = sales[1].tiers.iter().map(|t| t.figures.sold).collect();
    assert_eq!(per_tier, [3, 3]);

    // The curve ends on today's figures.
    let last = sales.last().unwrap();
    assert_eq!(last.total, book.tickets().total);
    assert_eq!(last.tiers[1].figures, figures(&book, regular));
}

#[test]
fn manual_sales_alone_make_a_curve() {
    let (mut book, early, _) = with_tiers();
    assert!(book.tickets().sales.is_empty());
    sale(&mut book, early, 2, 3);
    sale(&mut book, early, 1, 5);
    let sales = &book.tickets().sales;
    assert_eq!(sales.len(), 1);
    assert_eq!((sales[0].date, sales[0].total.sold), (june(5), 3));
}

#[test]
fn order_ids_stay_out_of_the_sales_over_time() {
    let (mut book, _, _) = with_tiers();
    import(&mut book, 1, &[("ORDER-4711", "Regular")]);
    let json = serde_json::to_string(&book.tickets().sales).unwrap();
    assert!(!json.contains("ORDER-4711"));
}

// --- profitability ---------------------------------------------------------------------------

/// A budget of 1.000 € for the kitchen, with `rows` of Regular sold at 32 € less 0,80 €.
fn with_budget(rows: &[(&str, &str)]) -> (Book, TierId, TierId) {
    let (mut book, early, regular) = with_tiers();
    let kitchen = expense(&book, "Kitchen");
    book.add_budget_line(BudgetLineDraft {
        category: kitchen,
        description: "Food".into(),
        estimate: Estimate::Amount {
            amount: eur("1.000"),
        },
        complete: false,
    })
    .unwrap();
    if !rows.is_empty() {
        import(&mut book, 1, rows);
    }
    (book, early, regular)
}

#[test]
fn there_is_no_break_even_without_a_budget_or_after_closing() {
    let (book, _, _) = with_tiers();
    assert_eq!(book.profitability(), None);
    let (mut budgeted, _, _) = with_budget(&[]);
    assert!(budgeted.profitability().is_some());
    let accepted = budgeted
        .ledger()
        .open_items()
        .into_iter()
        .map(|item| BlockerOverride {
            item,
            note: "not counted".into(),
        })
        .collect();
    budgeted.close(accepted, now()).unwrap();
    assert_eq!(budgeted.profitability(), None);
}

#[test]
fn break_even_counts_tickets_overall_and_per_tier() {
    let (book, early, regular) = with_budget(&[("1", "Regular"), ("2", "Regular")]);
    let p = book.profitability().unwrap();

    // Tickets bring in 2 × 31,20 €, the kitchen will cost 1.000 €.
    assert_eq!(p.projected, eur("-937,60"));
    assert_eq!(p.projected, book.budget().result.forecast);
    assert_eq!(p.gap, eur("937,60"));
    // 937,60 € at an average of 31,20 € is 30,05…
    assert_eq!(p.tickets, Some(31));

    let early = p.tiers.iter().find(|t| t.tier == early).unwrap();
    assert_eq!(
        (early.net_price, early.tickets, early.left),
        (eur("25"), 38, None)
    );
    // The quota of 10 has 8 places left: this tier alone cannot do it.
    let regular = p.tiers.iter().find(|t| t.tier == regular).unwrap();
    assert_eq!(
        (regular.net_price, regular.tickets, regular.left),
        (eur("31,20"), 31, Some(8))
    );
}

#[test]
fn a_refund_lowers_the_average_and_gives_the_place_back() {
    let (mut book, _, regular) = with_budget(&[("1", "Regular"), ("2", "Regular")]);
    import(&mut book, 2, &[("2", "Regular")]);
    book.resolve_conflict("1", regular, refund(1, 10_000, 3))
        .unwrap();
    let p = book.profitability().unwrap();
    // Net 2 × 32 − 2 × 0,80 − 32 = 30,40 € over 2 sold; the gap is 969,60 €.
    assert_eq!(p.gap, eur("969,60"));
    assert_eq!(p.tickets, Some(64));
    let t = p.tiers.iter().find(|t| t.tier == regular).unwrap();
    assert_eq!(t.left, Some(9));
}

#[test]
fn nothing_sold_gives_no_average_and_free_tiers_no_row() {
    let (mut book, _, _) = with_budget(&[]);
    let free = book.add_tier(tier("Helper", "0", 0, None)).unwrap();
    let p = book.profitability().unwrap();
    assert_eq!(p.tickets, None);
    assert!(p.tiers.iter().all(|t| t.tier != free));
    assert_eq!(p.tiers.len(), 2);
}

#[test]
fn a_forecast_that_is_not_negative_needs_no_more_tickets() {
    let rows: Vec<(String, &str)> = (0..40).map(|i| (i.to_string(), "Regular")).collect();
    let rows: Vec<(&str, &str)> = rows.iter().map(|(o, t)| (o.as_str(), *t)).collect();
    let (book, _, _) = with_budget(&rows);
    let p = book.profitability().unwrap();
    assert_eq!(p.projected, eur("248"));
    assert_eq!((p.gap, p.tickets), (Cents::ZERO, Some(0)));
    assert!(p.tiers.is_empty());
}

#[test]
fn estimates_on_the_ticket_categories_do_not_count_the_money_twice() {
    let (mut book, _, regular) = with_tiers();
    import(&mut book, 1, &[("1", "Regular"), ("2", "Regular")]);
    let fees = role(&book, Role::TicketFees);
    let tickets = role(&book, Role::Tickets);
    let plan = |category, description: &str, amount: &str| BudgetLineDraft {
        category,
        description: description.into(),
        estimate: Estimate::Amount {
            amount: eur(amount),
        },
        complete: false,
    };
    let fee_line = book.add_budget_line(plan(fees, "Fees", "50")).unwrap();
    let income_line = book
        .add_budget_line(plan(tickets, "Tickets", "5.000"))
        .unwrap();

    assert_eq!(figures(&book, regular).net, eur("62,40"));
    assert_eq!(book.budget().line(fee_line).unwrap().forecast, Cents::ZERO);
    assert_eq!(
        book.budget().line(income_line).unwrap().forecast,
        Cents::ZERO
    );
    // The estimates stay for plan vs actual, and the forecast is what the tickets brought in.
    assert_eq!(book.budget().line(fee_line).unwrap().estimate, eur("50"));
    assert_eq!(book.budget().result.forecast, eur("62,40"));
}

// --- whatever the data -----------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Whatever the sheets, their dates, the manual sales and the resolutions: the curve is in
    /// date order and ends on today's figures, and places never exceed what was sold.
    #[test]
    fn the_curve_ends_on_the_current_figures(
        sheets in prop::collection::vec(
            (prop::collection::vec((0..4u8, 0..2u8), 0..8), 1..28u8), 1..5),
        sales in prop::collection::vec((0..2u8, 1..4u32, 1..28u8), 0..4),
        refunds in prop::collection::vec((0..=10_000u32, 1..28u8), 0..8),
    ) {
        let (mut book, early, regular) = with_tiers();
        let mut refunds = refunds.into_iter();
        for (rows, day) in &sheets {
            let rows: Vec<(String, &str)> = rows
                .iter()
                .map(|(o, t)| (format!("O{o}"), if *t == 0 { "Early Bird" } else { "Regular" }))
                .collect();
            let rows: Vec<(&str, &str)> = rows.iter().map(|(o, t)| (o.as_str(), *t)).collect();
            import(&mut book, *day, &rows);
            let open: Vec<(String, TierId, u32)> = book
                .tickets()
                .conflicts
                .iter()
                .flat_map(|c| c.removed.iter().map(|r| (c.order.clone(), r.tier, r.count)))
                .collect();
            for (order, tier, count) in open {
                let Some((bp, day)) = refunds.next() else { break };
                book.resolve_conflict(&order, tier, refund(count, bp, day)).unwrap();
            }
        }
        for (i, (tier, count, day)) in sales.iter().enumerate() {
            let tier = if *tier == 0 { early } else { regular };
            let person = person(&mut book, &format!("Seller {i}"));
            let id = book.add_manual_sale(ManualSaleDraft {
                tier, count: *count, person, date: june(*day), reference: None,
            }).unwrap();
            if let Some((bp, day)) = refunds.next() {
                book.add_cancellation(CancellationDraft {
                    source: CancellationSource::Sale { sale: id },
                    count: 1,
                    terms: terms(bp, day),
                    note: None,
                }).unwrap();
            }
        }

        let tickets = book.tickets();
        let last = tickets.sales.last().unwrap();
        prop_assert_eq!(last.total, tickets.total);
        for (point, current) in last.tiers.iter().zip(&tickets.tiers) {
            prop_assert_eq!(point.tier, current.tier.id);
            prop_assert_eq!(point.figures, current.figures);
        }
        prop_assert!(tickets.sales.windows(2).all(|w| w[0].date <= w[1].date));
        prop_assert!(tickets.sales.iter().all(|p| p.total.places <= p.total.sold));
    }

    /// The projected result is the Overview's forecast, and the break-even count is exact.
    #[test]
    fn break_even_is_the_smallest_count_that_closes_the_gap(
        rows in prop::collection::vec(0..6u8, 1..10),
        estimate in 1i64..1_000_000,
    ) {
        let (mut book, _, regular) = with_tiers();
        let kitchen = expense(&book, "Kitchen");
        book.add_budget_line(BudgetLineDraft {
            category: kitchen,
            description: "Food".into(),
            estimate: Estimate::Amount { amount: Cents::new(estimate).unwrap() },
            complete: false,
        }).unwrap();
        let rows: Vec<String> = rows.iter().map(|o| format!("O{o}")).collect();
        let rows: Vec<(&str, &str)> = rows.iter().map(|o| (o.as_str(), "Regular")).collect();
        import(&mut book, 1, &rows);

        let p = book.profitability().unwrap();
        prop_assert_eq!(p.projected, book.budget().result.forecast);
        let tier = p.tiers.iter().find(|t| t.tier == regular);
        if p.projected.is_negative() {
            let t = tier.unwrap();
            let worth = |n: u32| t.net_price.get() * i64::from(n);
            prop_assert!(worth(t.tickets) >= p.gap.get());
            prop_assert!(worth(t.tickets - 1) < p.gap.get());
        } else {
            prop_assert!(tier.is_none());
            prop_assert_eq!(p.tickets, Some(0));
        }
    }
}
