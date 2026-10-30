# Lancelot — Requirements

Status: draft v0.9 · 2026-10-28

## 1. Purpose

Lancelot replaces the Excel sheet used to plan and settle events of a volunteer group. For a single event it must:

1. **Budget** the event in advance using estimated costs.
2. **Record** every real money movement, with receipts attached.
3. **Settle** the event: show the result, show who is owed what, and prove every cent is accounted for.
4. **Reimburse** volunteers who advanced money from their private funds — correctly and without forgetting anyone.

### Non-goals

- Legal, tax or formal bookkeeping compliance (GoBD etc.). There is no audit trail requirement beyond what helps the treasurer.
- Overhead or costs that do not belong to a specific event (settled outside Lancelot).
- Anything spanning multiple events: carried-over profit, cross-event balances, multi-year reports.
- Multi-user access. There is one user: the treasurer.

Mathematical correctness is **not** a non-goal. It is the top priority.

## 2. Guiding principles

- **One event = one isolated world.** An event knows nothing about events before or after it. Money comes in, money goes out, and at the end there is a final balance that stays there forever.
- **Correct by construction.** All money is integer cents. Every transaction balances. All balances are derived from transactions, never stored separately or edited by hand.
- **Fast entry.** Entering an advance with a receipt should take well under a minute.
- **Own your data.** Each event is a single, portable, open-format file.

## 3. Glossary

| Term | Meaning |
|---|---|
| **Event** | The unit of planning and settlement. Self-contained. |
| **Account** | A place where the event's money sits, or a party the event owes money to or is owed money by. |
| **Verein account** | The bank account of a related Verein. Most ticket revenue goes here, some invoices are paid from it, and reimbursements are paid out of it. The group does not control it directly. |
| **Cash box** | Physical cash. Persists between events in real life; each event sees only an opening and a closing count. An event can have several cash accounts (e.g. the box and a bar till). Cash in someone's pocket is not a cash account; it is that person holding event money. |
| **Person** | Someone who advances money, or holds event money (e.g. tickets sold outside the regular channels and paid to their private account), or both. Has bank details. The two amounts net against each other into one balance. |
| **Ticket tier** | A ticket type with a fixed price, e.g. "Early Bird €25". |
| **Ticket sheet** | The spreadsheet from the Verein listing tickets sold: an `.ods` file with one sheet and one row per ticket. Its `Ticket` column holds the tier name, and an order ID column identifies the order a ticket belongs to (one order can have several tickets). It also contains personal data that Lancelot does not need. It has no prices and currently no sale dates. It is the source of truth for ticket income. |
| **Cancellation** | A ticket that was sold and later cancelled, usually detected because it disappeared from the ticket sheet. It carries a **refund percentage** from 0 % to 100 %. The event keeps the rest of the ticket price. |
| **Import conflict** | A change in the ticket sheet that needs a decision from the treasurer, e.g. fewer tickets of a tier than in the previous import. |
| **Advance (Auslage)** | A payment from a person's private funds on behalf of the event. Creates a liability: the event owes the person. |
| **Reimbursement** | Paying an advance back to the person. Default source: the Verein account. |
| **Category** | The kind of income or expense. Every transaction line has one (Tickets, Kitchen, Mobility, Drinks, General, …). |
| **Budget line** | A planned income or expense with an estimated amount. |
| **Cost centre** | A sub-area of an event with its own profit and loss. Drinks is the first one. |
| **Commission goods (Kommissionsware)** | Goods taken from a supplier with the right to return unsold units. Only what is kept is paid for. |
| **Strich / Strichliste** | A tally mark one person makes per drink, recorded under their name. At the end of the event people pay according to their Striche, at one fixed price per Strich. |

## 4. Domain model

### 4.1 Double-entry underneath, simple forms on top

Every transaction moves an amount **from** one account **to** another. That makes it impossible for money to appear or disappear, and every balance becomes a derived, checkable number. The user never sees debit and credit. They fill in "who paid, for what, how much".

Account kinds:

| Kind | Examples | Balance meaning |
|---|---|---|
| **Asset** | Verein account, cash box | Event money sitting there |
| **Person** | One per person involved | Net balance. Positive: the event owes the person (advances exceed money held). Negative: the person holds event money and owes it to the event. Advances and held money cancel out automatically. |
| **Income** | Tickets, Drink donations, General | Total earned per category |
| **Expense** | Kitchen, Drinks, Pfand, Mobility, Venue, … | Total spent per category |
| **Opening balance** | One per event | The counterpart for opening counts (e.g. cash box at start) |

How the real flows map onto this:

| Real-world event | From → To |
|---|---|
| Ticket revenue arrives at the Verein account | Income: Tickets → Verein account |
| Ticket sold outside regular channels, paid to Bob's private account | Income: Tickets → Person: Bob |
| Bob forwards that money to the Verein account | Person: Bob → Verein account |
| Cash box opening count €80 | Opening balance → Cash box |
| Anna puts €200 of her own money into the cash box | Person: Anna → Cash box |
| Anna pays for food with her own card | Person: Anna → Expense: Kitchen |
| Food bought from the cash box | Cash box → Expense: Kitchen |
| Venue invoice paid by the Verein | Verein account → Expense: Venue |
| Anna reimbursed | Verein account → Person: Anna |
| Drink donations collected in cash | Income: Drink donations → Cash box |

(Arrows show the direction money moves. The implementation stores signed amounts. The arrows are only for readability.)

Example of netting: Bob received €300 of ticket money and advanced €120 for food. His balance is −€180, so Bob owes the event €180 and no reimbursement is due to him.

### 4.2 Event isolation

- **R-ISO-1** Each event lives in its own file and has no references to other events.
- **R-ISO-2** Asset accounts track only this event's movements, plus an optional opening balance. The Verein account's real total balance is irrelevant and never entered.
- **R-ISO-3** Every **cash** account gets an **opening count** at the start and a **closing count** at the end of the event. The difference between the closing count and the computed balance is shown as a discrepancy. A bank account can be counted too, but need not be.
- **R-ISO-4** Convenience features may *copy* data from another event file into a new one (a budget template, person details). After copying, the files are independent.

## 5. Functional requirements

### 5.1 Event

- **R-EVT-1** Create an event: name, date range, location, notes.
- **R-EVT-2** An event is either **open** or **closed**. A closed event is read-only. Closing (R-SET-7) and reopening are explicit actions and are recorded in the change log. There are no other states: the dates show where an event stands.
- **R-EVT-3** Create a new event from an existing one as a template. This copies accounts, categories, cost centres, budget lines and persons, but no transactions.
- **R-EVT-4** Event settings include the **default reimbursement account** (initially the Verein account).

### 5.2 Accounts

- **R-ACC-1** Configurable asset accounts per event, each either **bank** or **cash**. They are added, renamed and switched between bank and cash in Settings. Defaults: Verein account (bank), Cash box (cash). Money in persons' private bank accounts is not an asset account. It is tracked through the person's balance (§4.1).
- **R-ACC-2** Configurable income and expense categories. Defaults: income Tickets, Drink donations, General; expenses Kitchen, Drinks, Pfand, Mobility, Venue, Tech, General.
- **R-ACC-3** Every account shows its derived balance and its transaction history.
- **R-ACC-4** **Reconciliation checkpoints**: the user enters "statement shows X on date D" for an asset account. Lancelot flags any mismatch with the computed balance at D. For the Verein account this compares the net movement for the event, not the absolute balance.

### 5.3 Transactions

- **R-TX-1** Fields: date, amount, source account, one or more destination lines (account/category, amount, optional budget line, optional cost centre), description, counterparty (free text: shop or supplier), attachments, notes.
- **R-TX-2** **Split transactions.** One receipt can cover several categories, and the lines must sum exactly to the total.
- **R-TX-3** **Typed entry forms** for the common cases, so the treasurer never has to think about accounts:
  - *Expense*: who paid (an asset account or a person), what for, and how much.
  - *Income*: which category, where it arrived, and how much.
  - *Transfer*: between asset accounts, a person putting money into the cash box, or a person forwarding held money to the Verein account.
  - *Reimbursement*: which person and from which account (pre-filled with the default). The amount is pre-filled with the person's positive net balance.
- **R-TX-4** Reimbursements and repayments settle a person's **net balance**, not individual advances. Partial payments are possible.
- **R-TX-5** Each person shows a status: **owed by event / owes event / settled**, derived from the net balance.
- **R-TX-6** Transactions are editable until the event is Closed. Every create, edit and delete is written to a per-event change log (timestamp, before and after values).
- **R-TX-7** The ledger can be grouped **by day** or **by category**. Grouped by category, each category is a header with its total, and a split transaction appears under each of its categories with that line's amount.

### 5.4 Attachments

- **R-ATT-1** Zero or more attachments per transaction. Formats: PNG, JPG, HEIC, PDF, plus any other file stored as-is.
- **R-ATT-2** Attachments are stored **inside the event file**, so the file stays self-contained.
- **R-ATT-3** Preview in the app, and open or export the original.
- **R-ATT-4** A list of expense transactions that have no attachment ("missing receipts").
- **R-ATT-5** Drag-and-drop a file onto the app to start a new transaction with the file attached.

### 5.5 Budget (plan vs actual)

- **R-BUD-1** Budget lines per event: category, optional cost centre, description, and the estimate, either as an amount or as quantity × unit price.
- **R-BUD-2** Actual transaction lines can be linked to a budget line (many-to-one).
- **R-BUD-3** A plan-vs-actual view per line, per category, per cost centre and in total.
- **R-BUD-4** A **forecast** result: for each budget line, use the actual amount if it is marked complete, otherwise the cautious one of the estimate and the actual so far: the larger for an expense line, the smaller for an income line. Unbudgeted actuals are added on top.
- **R-BUD-5** Budget lines are planning data only and never affect real balances.

### 5.6 Ticket sales

- **R-TKT-1** **Ticket tiers** per event: name, price, fee percentage (default 0 %, see R-TKT-10), and an optional quota. They are stored in the event file and copied by templates (R-EVT-3). This is the tier→price mapping that the ticket sheet lacks.
- **R-TKT-2** Each tier has a list of **aliases**: the exact names used for it in the ticket sheet.
- **R-TKT-3** **Import the ticket sheet.** The primary format is `.ods`; `.xlsx` and `.csv` are also supported because the parser handles them for free. By default the first sheet is read and each row counts as one ticket. The tier comes from the `Ticket` column and the order from the `Order ID` column. The column names can be changed in the import settings (tier column, order ID column, plus an optional sale date column), in case the Verein's format changes. These settings are saved in the event and copied by templates.
- **R-TKT-4** Tier names in the sheet that match no tier or alias **block the import** until the user maps them. Rows are never silently skipped. Rows that cannot be parsed are listed.
- **R-TKT-5** **Re-import** an updated sheet: this replaces the previous import and shows a preview of what changed (e.g. "+14 Regular, −2 Early Bird") before confirming. For traceability, each import records the file name, a SHA-256 hash of the file, and the extracted rows. The file itself is **not** stored (R-TKT-17). Removed tickets create import conflicts (R-TKT-12).
- **R-TKT-6** Imported sales generate ticket income on the Verein account: one transaction per tier, worth count × tier price (gross). If the tier has a fee, the same transaction includes a line `Expense: Ticket fees` for the fee amount, so the Verein account receives the net. These transactions are marked *imported*, cannot be edited by hand, and are regenerated on re-import.
- **R-TKT-7** Tickets sold outside the regular channels are entered manually (tier, count, the person who received the money, and an optional free-text reference). They generate `Income: Tickets → Person` and count towards the same statistics. They never appear in the ticket sheet, so they never cause import conflicts.
- **R-TKT-8** **Sales statistics:** tickets sold over time (cumulative, per tier and in total), revenue over time, cancellations, and sold vs quota per tier.
  - **Now:** the sheet has no sale dates. Every import is stored as a **snapshot** of the counts per tier, dated with the import date. The user can change that date, e.g. when importing an older file after the fact. Imports count in the order of their dates: the one with the latest date is the current sheet, and an older file only adds to the curve. The time series is built from the snapshots, so regular re-imports produce a sales curve.
  - **Quota:** a cancelled ticket frees its place, whatever the refund, even if the order stays in the sheet. It still counts as sold for the money (R-TKT-15).
  - **Later:** if the Verein adds a sale date column, it can be configured in the import settings (R-TKT-3), and the statistics then use real sale dates. The design must keep this a configuration change: the statistics read from one "sales over time" source, which is fed either by snapshots or by per-row dates.
- **R-TKT-9** **Profitability:** current net ticket revenue against the forecast costs (R-BUD-4), giving a projected result. **Break-even** is shown as the number of additional tickets needed, both at the average net price sold so far and per tier.
- **R-TKT-10** **Fees:** each tier has an optional fee percentage, entered as a number with up to two decimals (e.g. `2.5`) and stored as an integer in basis points (`250`). The fee is calculated per ticket and rounded half-up to the cent, then multiplied by the count. This way the total is independent of how tickets are grouped and matches how platforms usually charge.
- **R-TKT-11** The ticket sheet is the **source of truth** for ticket income. There is no separate reconciliation of ticket income against amounts reported by the Verein. Cancellations (R-TKT-12…15) are the treasurer's documented interpretation of changes to the sheet.

#### Cancellations and refunds

- **R-TKT-12** **Detecting changes by order ID.** Each import is compared with the previous one per **(order ID, tier)**. That pair is the key, and its value is the number of tickets.
  - **Removed:** an order disappeared, or one of its tiers has fewer tickets than before. This creates an **import conflict**, e.g. "Order 4711: 2 × Regular removed".
  - **Added:** new orders, or more tickets in an existing order. These are imported normally and listed in the preview.
  - **Changed tier:** an order whose tickets moved from one tier to another (e.g. an upgrade) shows up as one removal plus one addition. The conflict view groups them by order, so they can be resolved together, normally as a correction.
  - Removals are detected exactly, even when other tickets of the same tier are sold in the meantime.
- **R-TKT-13** **Resolving a conflict.** Conflicts are listed in a dedicated place (a ticket conflicts view, with a badge in the navigation), grouped by order. Each conflict is resolved, wholly or split into parts, as one of:
  - **Cancellation with refund**: a refund percentage from 0 % to 100 % (default 100 %), a date and a note. 0 % means the ticket was cancelled but the event keeps the whole price.
  - **Match to an existing cancellation** that was already recorded manually (R-TKT-14).
  - **Correction**: the ticket was never really sold, e.g. a data error in the sheet. It produces no income and no refund.
- **R-TKT-14** **Manual cancellations.** A cancellation (tier, count, refund %, date, note) can also be recorded without a sheet change. It references either an **order ID** from the sheet (e.g. a partial refund agreed after a deadline while the order stays in the sheet) or a **manual sale** (R-TKT-7). When a sheet order with a manual cancellation later disappears, its conflict is matched to that cancellation automatically and only needs to be confirmed, so it is not counted twice.
- **R-TKT-15** **Calculating ticket money per tier**, all in integer cents:
  - **Sold count** = current sheet count + cancelled tickets that are no longer in the sheet.
  - **Gross income** = sold count × price.
  - **Refunds** = Σ over cancellations of count × per-ticket refund, where the per-ticket refund is price × refund % rounded half-up to the cent. They are booked as `Income: Ticket refunds` (negative income).
  - **Who pays the refund:** by default the account that received the money. That is the **Verein account** for tickets from the sheet, and the **person** recorded on the sale for manual sales (R-TKT-7). A refund from a person reduces the event money they hold, so it nets into their balance (§4.1). The payer can be overridden per cancellation, e.g. to the cash box.
  - **Fees** follow R-TKT-10 on the sold count. They are **not refunded** on cancelled tickets: the platform keeps them.
  - Corrections reduce neither the sold count nor anything else. They simply leave the ticket out.
- **R-TKT-16** Until a conflict is resolved, the removed tickets are treated as **not sold** (the sheet is the source of truth), and every ticket total shows a warning that there are unresolved conflicts.

#### Data minimisation

- **R-TKT-17** The ticket import reads **only** the configured columns: tier, order ID, and sale date once it exists. All other columns (names, e-mail addresses, …) are neither parsed into the event nor stored, and the original file is not embedded. The import dialog states which columns are read.
- **R-TKT-18** Order IDs are the only buyer-related data in the event file. Nothing in Lancelot links an order ID to a person's identity. Manual sales and cancellations hold only what the treasurer types in.

### 5.7 Persons

- **R-PER-1** Per event: name, account holder, IBAN, BIC (optional), contact, notes.
- **R-PER-2** Validate the IBAN checksum and format on entry.
- **R-PER-3** **Person overview:** per person, the total advanced, total money held, payments made (reimbursements and repayments) and the net balance, with a drill-down to each underlying transaction and its receipts.
- **R-PER-4** **Payout list** for the Verein's treasurer, exported as PDF and CSV. It includes every person with a positive net balance: account holder, IBAN, net amount, a suggested payment reference, and the itemised advances and held money as an optional appendix.
- **R-PER-7** A **repayment list** of persons with a negative net balance (they owe the event), showing the amount and the target account (default: the Verein account).
- **R-PER-5** After the payout list has been paid, the user can record all of its reimbursements in one action.
- **R-PER-6** Optional, later: export the payout list as SEPA pain.001 XML for batch upload.

### 5.8 Cost centres and Drinks

- **R-CC-1** Any transaction line or budget line can be tagged with a cost centre. Each cost centre has its own P&L: income minus expenses tagged to it.
- **R-CC-2** The **Drinks** cost centre is created by default.
- **R-DRK-1** Drinks are bought on commission. The supplier bills after the return, so the drinks cost is normally one invoice for the drinks actually kept.
- **R-DRK-2** Pfand is recorded as its own expense category (`Pfand`) in the Drinks cost centre. It only occurs for drinks that could not be returned because only full crates can be returned. Pfand on drinks that were sold and returned as empties is not tracked.
- **R-DRK-3** Drink revenue is recorded as income `Drink donations`, wherever it was collected (cash box, held account, Verein account).
- **R-DRK-4** The Drinks P&L shows: cost (drinks + Pfand), revenue (donations), and the result (profit or loss).
- **R-DRK-5** Optional per-product records: product, units taken, units returned, units kept, and price per unit. These let the tool check the supplier invoice and compute the cost per drink.
- **R-DRK-6** **Price per Strich** is a single event setting. It applies to all drinks.

### 5.9 Strichliste (optional, pluggable)

- **R-STR-1** A generic **Strichliste provider interface**, so that a provider for any service can be written later. Lancelot ships no concrete provider, only the interface and a manual CSV import as a reference implementation.
- **R-STR-2** A provider delivers a list of tallies: person name, count, product (optional), timestamp (optional).
- **R-STR-3** If tallies are present, the Drinks view additionally shows:
  - the expected revenue (Σ Striche × price per Strich, R-DRK-6) against the actual donations, and
  - per person, the Striche and amount due, with an optional "paid" checkbox.
- **R-STR-4** Strichliste data is informational only. It never creates transactions automatically, because the real money is what was actually collected.
- **R-STR-5** The whole feature can be turned off per event, which hides it from the UI.

### 5.10 Settlement report

- **R-SET-1** Per event: income by category, expenses by category, result.
- **R-SET-2** Plan vs actual summary.
- **R-SET-3** P&L per cost centre (Drinks).
- **R-SET-4** **Final balance:** the balance per asset account (Verein account net movement, and the cash box with its closing count and any discrepancy). This is the event's final state.
- **R-SET-5** **Open items:** persons with a non-zero net balance (in either direction), missing receipts, unresolved reconciliation mismatches, and unresolved ticket import conflicts.
- **R-SET-6** Export as PDF (human-readable, with the receipts) and CSV (raw transactions, one row per line).
- **R-SET-7** An event can only move to **Closed** when there are no open items. Each blocker can be overridden explicitly with a note.
- **R-SET-8** Once an event is closed, Lancelot shows its final balance indefinitely. What happens to the money afterwards (e.g. moving cash or surplus elsewhere) is outside Lancelot's scope and is not modelled.

## 6. Correctness requirements

- **R-COR-1** Amounts are stored and computed as **integer cents** (`i64`). No floating point anywhere in the money path, including the UI.
- **R-COR-2** Every transaction balances to exactly zero. This is enforced in the domain layer *and* by a database constraint or check.
- **R-COR-3** Splitting rule: when an amount is divided (e.g. quantity × unit price, or splitting equally), the remainder cents go to the first lines. Percentages (fees) are computed in integer arithmetic and rounded half-up (R-TKT-10). The rules are deterministic and documented.
- **R-COR-4** All balances and reports are derived from transactions. No cached totals unless they are verified on load.
- **R-COR-5** An **integrity check** runs on every open and save, and on demand: every transaction balances, no line points to a missing account, attachments are intact (by hash), and reimbursements do not exceed their advances. Failures are shown prominently.
- **R-COR-6** All calculation logic lives in a single core module with extensive unit tests and property-based tests (e.g. "for any sequence of valid transactions, the sum of all account balances is 0").
- **R-COR-7** Single currency: EUR.

## 7. Non-functional requirements

- **R-NF-1** Local desktop application. Primary platform macOS, with Windows and Linux possible.
- **R-NF-2** Fast: the app starts in under 1 second, and every interaction responds instantly at expected data sizes (≈ 40–100 transactions and ≈ 100 attachments per event).
- **R-NF-3** Works fully offline.
- **R-NF-4** The event file is a single file in an open format, readable without Lancelot.
- **R-NF-5** Automatic backup: keep the last N versions of each event file next to it, or in an app data directory.
- **R-NF-6** UI language: English. The **number and date format** is a user setting (app-wide, not per event). Options: `1.234,56 €` / `31.12.2026`, or `€1,234.56` / `2026-12-31`. The default follows the OS locale.
- **R-NF-7** Keyboard-friendly entry for the treasurer, which matters when entering a stack of receipts.
- **R-NF-8** **Lenient amount input.** Amount fields accept both formats regardless of the display setting, as long as the input is unambiguous:
  - The last `.` or `,` followed by 1–2 digits is the decimal separator: `12,5` = `12.5` = €12.50, and `1.234,56` = `1,234.56` = €1,234.56.
  - Any other separators are thousands separators and must form correct groups of three. `1.23.4` is rejected.
  - More than two decimals are rejected; nothing is silently rounded.
  - **Ambiguous case:** a single separator followed by exactly three digits (`1.234`, `1,234`) is read according to the user's format setting, and the hint below says so.
  - The field always shows a live preview of the interpreted value (e.g. "= €1,234.00") before it is committed. Invalid input is flagged and cannot be saved.
  - The currency sign and spaces are ignored. A leading `-` is allowed only where negative amounts make sense.

## 8. Proposed architecture

**Tauri 2 + SvelteKit (static SPA) + Rust core + SQLite** fits well:

| Layer | Choice | Why |
|---|---|---|
| Shell | **Tauri 2** | Small binary, fast start, native file dialogs and drag-and-drop. Much lighter than Electron. |
| UI | **SvelteKit** with `adapter-static` (SPA mode), TypeScript | Fast, little boilerplate, a good fit for form-heavy UIs. No SSR is needed in a desktop shell. |
| Domain core | **Rust crate** (`lancelot-core`), independent of Tauri | All money logic in one strongly typed, testable place. The UI only displays results and sends commands. |
| Storage | **SQLite**, one `.lancelot` file per event, attachments as BLOBs | One portable file, transactional writes, open format, trivial backup. |
| Money type | `Cents(i64)` newtype in Rust. Crosses the IPC boundary as an integer, formatted for display in the UI only. | Rules out float errors by construction. |
| PDF export | Generated in Rust (e.g. `typst` as a library) or from HTML via the webview's print-to-PDF | To decide in the design phase. |
| Strichliste | A Rust trait `StrichlisteProvider` in core, with a CSV implementation | Matches R-STR-1. |
| Excel import | `calamine` crate in Rust (reads .xlsx/.xls/.ods) | Pure Rust, read-only, no Office installation needed. |
| Charts | A lightweight Svelte-friendly chart library (e.g. LayerChart or uPlot) | Sales over time and plan vs actual. To decide in the design phase. |

Alternatives considered:
- **Electron**: heavier, and the money logic would live in JS.
- **Pure native (SwiftUI)**: macOS only.
- **Pure Rust UI (egui/iced)**: less polished for form-heavy, document-like UIs.

The Tauri approach has one real cost: the IPC boundary between Svelte and Rust. It is acceptable if the commands are coarse-grained (e.g. `create_transaction(dto) -> Result<EventSnapshot>`).

## 9. Phasing

1. **MVP**: event file, accounts, typed transaction entry with splits, attachments, persons with IBAN and netting, reimbursements and repayments, person overview, payout list, settlement report, integrity check. Ticket income is entered manually.
2. **Phase 2**: ticket tiers and ticket sheet import, budget and forecast, sales statistics and profitability, cost centres and Drinks P&L, per-product drink records, reconciliation checkpoints, templates from previous events.
3. **Phase 3**: Strichliste interface and CSV import, SEPA XML export, polished PDF reports, receipt OCR.

## 10. Remaining open points

These do not block the start. They can be settled during design:

1. **Sale dates** (pending, with the Verein): if the sheet gains a purchase date column, switch R-TKT-8 to real dates.
2. **Fee refunds** are assumed not to happen (R-TKT-15). Revisit if a platform with fees is ever used.

Settled in v0.4: ticket sheet format (`.ods`, `Ticket` column), fees (per-tier percentage, default 0), sheet as source of truth, manual entry of Verein-paid invoices, and the number format as a user setting with lenient input.
Settled in v0.5: import-date snapshots for the sales chart, and cancellations with flexible refund percentages resolved from import conflicts.
Settled in v0.6: change detection by order ID, data minimisation for the ticket sheet, fees not refunded, and refunds paid by whoever received the money (Verein or person), overridable.
Settled in v0.8: no Planning/Running/Settling states, only open and closed; new default categories; the ledger grouped by day or by category.
