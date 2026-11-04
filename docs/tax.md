# Lancelot — Tax (consideration)

Status: consideration, nothing decided · 2026-11-04

What it would take for Lancelot to know about tax, mainly VAT: where event hosts meet it, what it would change in the core, the plugins, the reports and the docs, and which workflows it would open. It follows the earlier idea of turning drink “donations” into revenue with a VAT rate per product, and asks how far beyond that plugin to go.

Today tax is a non-goal (requirements §1, manual “Who it is not for”). Every amount is booked as it was paid, which is gross, and the result is gross too.

> The tax rules below are examples to show what the app would have to model, mostly German because the current users are. They are not advice, and they need checking with the association’s treasurer or a tax advisor before any of it is built.

## 1. Who would need it

Whether tax matters depends on who stands behind the event, not on the event itself. Four kinds of host:

| Host                                                                                     | VAT                                                                                                                                                                                | What they would need from Lancelot                                                                                                                                                                                                     |
| ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A. Group under an association that charges no VAT** (small-business rule, e.g. §19 UStG) | None on sales, and none to reclaim on purchases.                                                                                                                                   | No VAT figures, but revenue sorted the way the association reports it: what counts as business revenue, what is a real donation. Revenue totals count towards the association’s yearly limits.                                           |
| **B. Group under a VAT-registered association**                                           | On tickets, drinks and food, at different rates. Input VAT on purchases may be reclaimable.                                                                                         | Net and VAT per rate on the revenue side, input VAT from receipts, and a summary the association’s treasurer can take into their return. The event’s result should be net, or it overstates what the group made by the VAT it owes. |
| **C. Commercial organiser** (a company, a sole trader running parties)                    | Full VAT, always.                                                                                                                                                                  | Same as B, and the result is what feeds income tax. Lancelot would still be a tool for one event beside their real books.                                                                                                              |
| **D. Hosts outside the EU model**                                                          | US sales tax is added on top of the price at the till and varies by place. Switzerland and the UK work like the EU, with their own rates and thresholds. | A rate that is _included_ in the price (EU, CH, UK) is a split of what was collected. A tax _added on top_ (US) changes what is collected. Only the first fits the current model without new entry forms.                           |

The current users are most likely A, possibly B. A and B are worth designing for. C comes for free with B. D's added-on-top tax stays out.

## 2. Where in the workflow tax shows up

Following an event from planning to closing, these are the moments a host would meet tax.

### Planning

- **Ticket price.** A 25 € ticket at 7 % brings in 23,36 € net, before platform fees. Break-even (R-TKT-9) and the average net price per ticket would have to leave the VAT out, or the missing amount is understated.
- **Budget lines.** Estimates would be net when the event is net (B, C) and gross otherwise (A). A budget line with quantity × unit price needs to say which. Mixing them makes the forecast (R-BUD-4) wrong by the VAT.
- **Pricing drinks.** Setting a price per product with a margin over cost is only meaningful net of VAT. Stock and sales could show the net margin per unit.

### Entering money

- **Expenses with receipts.** A receipt shows gross and the VAT per rate. For B, the VAT is reclaimable, so the expense is the net and the VAT is a claim on the tax office. A supermarket receipt can mix rates (food 7 %, cleaning supplies 19 %), which is a split transaction (R-TX-2) where each line has its own rate. The “well under a minute” goal holds only if the rate comes from the category by default and the VAT is calculated, with the printed amount typed in only when it differs.
- **Income.** Cash takings at the bar are one amount, but the drinks and the food in it may have different rates. They cannot be split after the fact unless something says how much of each was sold. This is where the **logged counts of Stock and sales** become the basis for the split (§4.2).
- **Ticket imports.** The generated income (R-TKT-6) would split into net and VAT per tier. Refunds (R-TKT-15) reduce the VAT by the same rate. Platform fees carry their own VAT, and a platform abroad may invoice it as reverse charge, which is a case to name and leave out at first.
- **Real donations.** Money given without anything in return has no VAT and is not business revenue. A fixed amount “donated” per drink is something in return, so tax offices generally treat it as a price. This is the real reason behind the rename in [§4.2](#42-stock-and-sales): the word changes nothing for the tax office, and a group should not believe it does.

### Settling and handing over

- **Result.** For B and C the result is net: VAT collected is owed to the tax office, VAT paid is owed back. Today’s result would be too high by the difference.
- **VAT summary.** Output VAT and input VAT per rate, and the difference, as a section of the settlement report and as rows in the CSV. Like the payout list (R-PER-4), it is something the event hands to the association’s treasurer, who files the return for the whole year. Lancelot never files.
- **Revenue by tax category.** For a non-profit, revenue belongs to one of four areas (in Germany: non-profit activity, asset management, purpose-related business, other business). Bar sales are usually “other business” and count towards a yearly limit. An event can’t know the yearly total (R-ISO-1), but it can say how much it contributed.

## 3. Two ways to build it

### Option 1: Informational only

Rates sit on categories, ticket tiers and products. The ledger stays as it is, all gross. The report and the CSV show, per rate, the gross amount and the VAT it contains. Like Stock and sales today, it never changes a transaction and an event settles the same without it.

- **Serves:** A fully (they need the sorting, not the VAT), and B as far as handing figures over goes.
- **Costs:** a rate list per event, a rate on categories, tiers and products, a report section and CSV columns. No change to entry forms, balances or the rounding rules beyond one formula.
- **Falls short:** the result, the cost centre P&L and break-even stay gross. Input VAT on a receipt with mixed rates can only be estimated from category defaults, not taken from the receipt.

### Option 2: VAT in the ledger

The event gets a switch, off by default, like Tickets (R-TKT-19): **Prices include VAT**. On, every income and expense line carries a rate, and the core books the VAT to its own account.

```
Anna pays 119 € for food with her card
  Person: Anna        → Expense: Kitchen   100 €
                      → Input VAT           19 €

Bar takings of 238 € in the cash box
  Income: Drinks      → Cash box           200 €
  Output VAT          → Cash box            38 €
```

- **Persons and assets don’t change.** Anna advanced 119 €, so she is owed 119 €. VAT only splits the category side of a transaction, so balances, payouts, counts and statements stay as they are. This is the property that makes Option 2 feasible.
- **The result becomes net by itself.** The VAT accounts are neither income nor expense, and `flow_of` and the result only count income and expense lines, so they already leave VAT out.
- **Serves:** B and C fully, and A gets Option 1’s report too.
- **Costs:** everything in §4.

**Suggested order:** Option 1 first, as it’s useful for every host and doesn’t touch the money path. Option 2 when a group under a VAT-registered association actually uses Lancelot. Option 1’s rate list and rates on categories, tiers and products are what Option 2 builds on, so nothing is thrown away.

## 4. What would change

### 4.1 Core

| Area                   | Option 1                                                                                 | Option 2, on top                                                                                                                                                                                                                                                                                                      |
| ---------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Rates**              | A list per event: name and rate in basis points, like fees (R-TKT-10), e.g. “Reduced 7 %”, “Standard 19 %”, “None 0 %”. No built-in law: templates copy the list, so a group sets it up once. | Unchanged.                                                                                                                                                                                                                                                                                                            |
| **Categories**         | A default rate per income and expense category.                                           | Expense categories also say whether input VAT is reclaimable, because spending for the non-profit side may not be.                                                                                                                                                                                                     |
| **Accounts**           | None.                                                                                     | Two new roles next to `Tickets`, `TicketFees` and `TicketRefunds`: `OutputTax` and `InputTax`, as accounts of a new kind that is neither income nor expense. Or one `Tax` account, which is simpler but loses the split the summary needs.                                                                                 |
| **Lines**              | None.                                                                                     | `SplitLine` gets a rate. The VAT is its own line in the transaction, stored, not derived on read, so the receipt’s printed figure can win over the calculated one and every balance stays a sum of stored lines (R-COR-4).                                                                                            |
| **Rounding** (R-COR-3) | VAT contained in a gross amount: `gross × rate ÷ (10 000 + rate)`, rounded half-up to the cent, per line. | Per line, not per receipt, so a split doesn’t change the total VAT depending on how it is split. The rule is documented and property-tested, as for fees. Invoices that round per receipt differ by a cent or two, which is why the printed amount can override.                                                      |
| **Entry forms**        | None.                                                                                     | Expense and Income show the rate per line, prefilled from the category, and the VAT it gives. Typing gross stays the default, because receipts are gross. Transfer and Settle never carry VAT.                                                                                                                       |
| **Tickets**            | A rate per tier.                                                                          | The generated transaction per tier gets a VAT line, so the ticket account still receives gross minus fee. Refunds take back VAT at the tier’s rate. Fees could carry input VAT. Every generated transaction is rebuilt on import, so old events need no migration of their ticket entries.                         |
| **Integrity** (R-COR-5) | None.                                                                                    | VAT lines only on VAT accounts, a VAT account only in income and expense entries, and a warning, not a failure, when a stored VAT is more than a few cents off its rate.                                                                                                                                               |
| **Budget**             | None.                                                                                     | Estimates are net when the switch is on. A budget line could take a gross estimate and a rate, to save the treasurer the division.                                                                                                                                                                                     |
| **Break-even** (R-TKT-9) | Optional: show the VAT per ticket.                                                     | Uses the net price per ticket after VAT and fees.                                                                                                                                                                                                                                                                      |
| **File format**        | A rate table, a rate column on accounts, tiers. Plugin records hold the products’ rates.  | A rate column on lines and two roles. The format isn’t frozen yet (technical design §3.2), so adding them now is an edit to `0001_initial.sql`. After the first real event, it is a migration of nullable columns, which is cheap too. There is no need to rush the schema for this.                                  |
| **Switching on later** | Free: only figures change.                                                                | Turning it on for an event with entries means old lines have no VAT split. Either it is only allowed while there are no income or expense entries, the way “Rows have an order ID” is fixed once there are imports, or it offers to split them all at their category’s rate, which is a big hidden change. The first is safer. |

### 4.2 Stock and sales

This is where it started.

- **Wording.** `donation` in `drinks/records.ts` becomes `price`. The cost centre chooses whether it asks for a **price** or a **donation**, since some groups collect donations on purpose. That choice changes labels only, such as “Expected revenue” or “Expected donations”. The default category “Drink donations” becomes “Drinks income”, as R-DRK-3 already calls it. The glossary’s _Tally_ entry and R-DRK-5, R-DRK-7 follow.
- **A rate per product**, defaulting to the centre’s income category rate. Prices are gross.
- **Expected revenue, gross and net**, against what was collected. With Option 1 this is all it does.
- **Splitting takings by rate (Option 2).** When the bar’s cash box takes drinks and food at different rates, the logged counts × price per rate give the share of each rate. The plugin could propose an income entry with one line per rate from those shares (`proposeEntry` already takes several lines). It stays a proposal, and the treasurer saves it, so the rule that the plugin never books anything on its own holds.
- **Net margin per unit**, cost against net price, where today it would compare a net cost with a gross price.
- **Context.** Plugins need the rate list and the rates of categories: `ctx.rates`, and a rate on the categories the context already lists. `ProposedLine` gets an optional rate.

### 4.3 Report, CSV and other outputs

- **Settlement report** (§5.10): a **Tax** section with, per rate, net, VAT and gross on each side, and the difference. With Option 1 it is titled as contained VAT, for information. The income and expense tables say whether they are net or gross.
- **CSV** (`export.rs`): columns `Rate` and `VAT`, empty when there is none. This is what the association’s books would import. A DATEV export is a separate, later question.
- **Payout list**: unchanged, as people are reimbursed what they paid.
- **“Where your ticket went”** ([future](future.md#where-your-ticket-went)): could show the share that went to VAT, which is a true and useful line on such a poster.
- **Templates** (R-EVT-3): copy the rate list, category rates, tier rates and the switch.

### 4.4 Docs

- **Requirements §1, non-goal.** Reword from “Legal, tax or formal bookkeeping compliance” to something like: _Filing taxes and formal bookkeeping compliance (GoBD etc.). Lancelot can record VAT and sort revenue so that the association’s books can take the figures, but those books stay the legal record._ The distinction matters because entries stay editable until closing (R-TX-6), which an official bookkeeping system can’t allow, and Lancelot shouldn’t pretend otherwise.
- **Manual**: “Who it is not for” says the same, and a new section explains the switch and the report.
- **Requirements**: a §5.11 “Tax” with its own IDs, and the touched IDs (R-TKT-6, R-TKT-9, R-TKT-15, R-DRK-5, R-DRK-7, R-SET-1, R-SET-6, R-COR-3) amended. The glossary gains _Rate_, _Net_, _Gross_ and _VAT_.
- **Technical design**: the rounding formula under §2.2, the accounts under §3.

## 5. Workflows it would open

- **Handing figures to the association.** Today the association’s treasurer gets the payout list and takes the result as one number. With a tax summary and rate columns, they get everything their return and their books need for this event, without re-sorting receipts.
- **Groups that are not purely volunteer.** Commercial organisers and VAT-registered associations could use Lancelot for their events, where today the gross result would mislead them.
- **Honest drink pricing.** Price, cost and VAT per product, so a group can see whether a 2,50 € beer still covers itself at 19 %.
- **Revenue per tax area.** Tagging revenue with its area (non-profit, business, …) would let an association add up its events against yearly limits outside Lancelot. Cost centres already come close, and a cost centre could carry its area, which is cheaper than a new concept.
- **Card and PayPal settlements** ([future](future.md#card-and-paypal-settlements)): the fee from SumUp, Stripe or PayPal would be booked with its own rate, often none.

## 6. Nearby obligations that are not VAT

Events meet other taxes and levies. None of them are planned. They’re named here so they are decided deliberately and not discovered later.

- **Withholding tax for performers from abroad** (in Germany §50a EStG). The event withholds part of the fee and pays it to the tax office. In Lancelot it would be an expense split into the artist’s share and a liability, much like VAT in Option 2.
- **Artists’ social insurance levy** (in Germany Künstlersozialkasse): a percentage on fees paid to artists, reported yearly by the association. A report line of fees to artists would help.
- **Allowances for volunteers** (in Germany the Ehrenamts- and Übungsleiterpauschale): tax-free up to a yearly amount per person, tracked by the association. Persons in Lancelot are only about advances today, and paying people is a different feature.
- **Donation receipts.** Real donations may need a receipt for the donor, which needs the donor’s name and address. That conflicts with data minimisation (R-TKT-18) and belongs to the association.

## 7. Questions to settle first

1. **Who stands behind the current users’ events:** an association with or without VAT? That decides whether Option 2 is needed at all.
2. **What does the association’s treasurer want handed over:** gross figures with rates, or net and VAT? Asking them is cheaper than guessing.
3. **Is reclaimable input VAT per category enough,** or does it differ per receipt?
4. **Price or donation for the drinks:** do any groups collect real donations at the bar, or is it always a fixed amount per drink?
5. **Wording of the non-goal:** is “record, don’t file” the line we want?
