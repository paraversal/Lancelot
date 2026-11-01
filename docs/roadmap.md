# Lancelot — Roadmap

Status: v0.10. M0 to M4 are done, and M5 is done except the parallel run (M3's exit test, a stack of receipts by keyboard, is yours to try). M6 is done except matching the Budget page to its design, and M7 is done: the ticket core, the Tickets screen and conflicts. M8 is done: the statistics core, the Tickets page and the Overview. M9 is done: cost centres and the Drinks page, which became a plugin in Stage 7. M10 is done: reconciliation checkpoints, templates and copying from another event; the Phase 2 exit still needs the next event. The [refactoring](#refactoring) is done, apart from one decision that waits for that event. The roadmap builds on [requirements v0.12](requirements.md) and the [design notes](design/README.md), and what exists today is listed in [features](features.md). Requirement IDs (R-…) are listed for each milestone, so you can trace every one.

## How we work

- **Core first, UI second.** Every rule about money lives in `lancelot-core` and is tested there before any screen uses it (R-COR-6). The UI only formats values and sends commands.
- **Vertical slices.** From M3 on, each milestone ends with something the treasurer can actually use, not with a finished layer.
- **Definition of done** for each milestone:
  - `cargo test` passes, including the property tests.
  - `cargo clippy -D warnings` passes, and `svelte-check` and the UI tests pass.
  - The integrity check passes on the milestone's test event.
  - The screens match `UI.fig`. Deviations are noted in `docs/design/README.md`.
  - `docs/features.md` lists what was added or removed.
- **A real event as the yardstick.** The MVP must be usable for the next event, run **in parallel with the Excel sheet**. The result is a release only when both arrive at the same final balance.

## Repository layout (target)

```
Cargo.toml                 workspace
crates/lancelot-core/      domain, storage, import, reports (no Tauri dependency)
app/                       SvelteKit SPA (adapter-static, TypeScript)
crates/lancelot-service/   commands on the open event, settings, API types (no Tauri dependency)
crates/lancelot-bridge/    development only: the commands over local HTTP for a plain browser
app/src-tauri/             Tauri 2 shell: commands, dialogs, drag-and-drop
docs/                      features, requirements, roadmap, future, design, technical design
fixtures/                  ticket sheets (.ods), receipts, format cases; the sample event
```

---

## Phase 1 — MVP

### M0 · Technical design and scaffold

Decide everything that is expensive to change later, then set up an empty project that builds.

- [x] `docs/technical-design.md`:
  - [x] SQLite schema: event, accounts, transactions, lines, attachments, persons, change log and meta/schema version, plus the balance CHECK or trigger (R-COR-2).
  - [x] Core API: commands, queries, invariants, and the error types.
  - [x] IPC contract: coarse-grained commands that return snapshots or view models.
  - [x] File handling: opening, the single-writer lock, atomic save, backups (R-NF-5) and schema migrations.
  - [x] The rounding and splitting rules written out with examples (R-COR-3).
- [x] Decisions, recorded in the technical design:
  - **PDF export:** the print dialog for now; Typst stays an option for M13.
  - **Charts:** hand-written SVG, to be revisited in M8.
  - **HEIC and PDF preview:** native WebKit.
  - **Where the amount parser runs:** Rust behind IPC, with a shared test-vector file.
- [x] Scaffold the workspace, Tauri 2 and SvelteKit. Bundle Familjen Grotesk locally (it must work offline, R-NF-3).
- [x] CI with GitHub Actions: fmt, clippy, tests, `svelte-check`, and a macOS build. Written, but it has not run on GitHub yet.
- [x] Load `tokens.css` into the app and render an empty window with the sidebar.

**Exit:** `pnpm tauri dev` opens a window that shows the empty sidebar in the right font and colours, and CI passes.

### M1 · Ledger core (Rust only, no UI)

This is the heart of the app. It gets the most tests. It is built in three steps, each in its own pull request.

**M1a · Money building blocks**

- [x] `Cents(i64)` with checked arithmetic, `allocate(total, n)` with remainder cents going to the first parts, and `percent_bp` rounding half away from zero. Covers R-COR-1 and R-COR-3.
- [x] Amount parser and formatter. Both formats, rejecting invalid grouping, the ambiguous three-digit case read by the setting, and the sign rules. 64 cases in `fixtures/amount-input.json`. Covers R-NF-8 and R-NF-6 (numbers; dates follow in M2).
- [x] IBAN validation (SEPA lengths, mod 97) and BIC format. Covers R-PER-2.

**M1b · Ledger model**

- [x] Accounts by kind: asset, person, income, expense, opening balance. Default accounts and categories. Covers R-ACC-1 and R-ACC-2.
- [x] Transactions with split lines, balanced by construction. Covers R-TX-1 and R-TX-2.
- [x] Typed builders: expense, income, transfer, reimbursement/repayment. They map the form inputs to lines, following the flow table in §4.1. Covers R-TX-3.
- [x] Derived balances: per account, per person net balance and status, and totals per category. Covers R-ACC-3, R-TX-4, R-TX-5 and R-COR-4.
- [x] The example event from `UI.fig`, built in memory: result **+2.027,50 €**, with matching category totals, account balances and person balances (`tests/mock_event.rs`).
- [x] Property tests: balances sum to 0, net balance equals the person's account balance, where the money is equals the result, editing equals deleting and adding.
- [x] Closing and reopening the event. Closed is read-only, and closing needs open items resolved or overridden with a note. Covers R-EVT-1, R-EVT-2, R-EVT-4 and R-SET-7. (The earlier Planning/Running/Settling states were removed: nothing used them.)

**M1c · Storage and integrity**

- [x] Change log with before and after values for every create, edit and delete. Covers R-TX-6.
- [x] SQLite storage:
  - [x] Create and open a `.lancelot` file.
  - [x] Atomic writes: one SQL transaction per command, and creation through a temporary file.
  - [x] Attachments as BLOBs with SHA-256 (R-ATT-1, R-ATT-2).
  - [x] Schema version and migrations.
  - [x] Backups of the last N versions (R-NF-5).
- [x] Cash box opening and closing counts with the discrepancy (R-ISO-3). Missing receipts and the closing count become open items (R-SET-5).
- [x] Integrity check on open, on save and on demand. Covers R-COR-5.
- [x] Tests:
  - [x] Unit tests for each requirement.
  - [x] Property tests with `proptest`, for example: all balances sum to 0, splits sum to the total, parse(format(x)) = x, and the integrity check holds after any valid sequence of operations.
  - [x] The example event from M1b is saved, reopened and still reaches **+2.027,50 €**.

**Exit:** a CLI or test can create the mock event, save it, reopen it and print correct balances. The core has no TODOs in the money path. ✓ `tests/store.rs` (`the_example_event_survives_saving_and_reopening`).

### M2 · App shell and design system in code

Built in two steps, each in its own pull request.

**M2a · Connecting the app to the event file**

- [x] Dates in both formats, display and input (R-NF-6), with `fixtures/date-input.json`.
- [x] `EventSnapshot`: every figure the UI shows, in one value.
- [x] TypeScript types generated from Rust (`ts-rs`), checked in CI.
- [x] Shell commands for settings, recent events, create/open/close, event details, the default reimbursement account, the integrity check, backups and restore, amount and date parsing. Logic in `service.rs`, tested without Tauri.
- [x] App settings in `settings.json`: number format (default from the OS locale), backup count, recent events.
- [x] UI: typed command wrappers, the session state, and `formatAmount`/`formatDate` tested against the shared fixture.

**M2b · Screens and components**

- [x] **Design the start screen, the new-event panel and the Settings screen** in `UI.fig`, plus the integrity banner and the "already open" and "damaged" states.
- [x] Svelte components from `docs/design/README.md`, on a development-only `/design` page that mirrors the System frame:
  - [x] Button, filter chip, segmented control, text field, select, date field
  - [x] **Amount input with live preview** (calls the core parser)
  - [x] Balance cell, receipt indicator, table with day groups, right-hand panel, stacked bar
- [x] A start screen with recent events, plus New and Open, using native file dialogs.
- [x] The event window: sidebar with the event, its state and dates, navigation with counts and attention badges, and the file name.
- [x] Settings: number/date format (R-NF-6), backup count and the backup list with restore, event details and the default reimbursement account, the file and its integrity check.
- [x] A keyboard model: ⌘O, ⇧⌘N, ⌘, , ⌘W as native menu items, ⌘1–3 and Esc (R-NF-7).
- [x] An integrity banner with "Check again" (R-COR-5).
- [x] A development-only mock backend, so the screens can be checked in a browser.

**Exit:** you can create, open and rename an event and switch the number format. The amount input behaves as it does in the System frame. ✓ Checked in the browser against the mock backend; the native dialogs and menu still need a click-through in the app.

### M3 · Ledger

The first slice the treasurer can use: entering the stack of receipts. Built in two steps.

**M3a · Entering and editing**

- [x] Designs: the Income, Transfer and Settle panels, deleting, adding a person or category on the spot, the empty ledger.
- [x] Ledger table: grouped by day or by category with totals (R-TX-7), type chips with counts, a No receipt chip (R-ATT-4), and a search over titles, people, categories and amounts.
- [x] Entry panel with the Expense / Income / Transfer / Settle switch, split lines with the amount left, the payer's live balance, and Settle filled in from the person's balance.
- [x] People and categories added from the panel's selects (people by name; details in M4).
- [x] Editing and deleting (with a confirmation), every change in the change log.
- [x] Keyboard: N, ↑/↓/Enter, ⌘↵, ⇧⌘↵ (save and next, keeping date and payer), Esc.
- [x] The core checks a split against the receipt total (`TxnDraft.total`).

**M3b · Attachments**

- [x] Designs: the receipt preview and the drop overlay.
- [x] Attachments: add (pick or drop), preview (image, PDF and HEIC through WebKit, via `lancelot://`), open the original in its app, and export. Saved in one SQL transaction with the transaction. Covers R-ATT-1 to R-ATT-3.
- [x] Drag files onto the window to start a new expense with them attached, or onto an open entry to attach them (R-ATT-5).

The cash box counts (R-ISO-3) moved to M5, next to the Overview's cash box row.

**Exit:** the 30–40 receipts of a past event can be entered with the keyboard alone in under 30 minutes.

### M4 · People and payouts

Built in two steps.

**M4a · People screen**

- [x] Design: editing a person in the right-hand panel.
- [x] Person records with IBAN validation as you type (checksum and length) and an IBAN warning in the list for people who are owed money. Covers R-PER-1 and R-PER-2.
- [x] People table: advanced, holds, settled and balance, with a totals row. A person panel with bank details, the open balance and the running balance; each line opens its transaction in the ledger (R-PER-3).
- [x] Settle from the person panel, filled in from the balance (R-TX-4).

**M4b · Payout and repayment lists**

- [x] Designs: the Payouts page and "Mark as paid".
- [x] Payout list with account holder, IBAN, a SEPA-safe reference and the amount; export as CSV (in the number format) and PDF (print layout, saved from the print dialog), with the optional itemised advances (R-PER-4).
- [x] Repayment list, with "Repaid" per person when the money arrives (R-PER-7).
- [x] "Mark as paid": record all of the payout list's reimbursements that have an IBAN in one action (R-PER-5).

**Exit:** for the fixture event, the payout list says Anna 212,40 € and Malte 110,00 €, and the repayment list says Bob 180,00 €. One click settles both payouts.

### M5 · Settlement and closing (MVP release)

- [x] Asset accounts are bank or cash, managed in Settings (R-ACC-1).
- [x] Opening and closing count with the difference for every cash account (R-ISO-3), from its row on the Overview.
- [x] Overview screen, MVP version: result, in/out by category, "where the money is" and open items, each leading to its fix. The forecast and ticket blocks wait for Phase 2.
- [x] Settlement report: income and expenses by category, result, final balance per asset account with its counts, open or accepted items, every entry, and the receipts one per page (PDF receipts drawn with pdf.js). Printed to PDF from the print dialog. Covers R-SET-1, R-SET-4, R-SET-5 and R-SET-6.
- [x] Transactions CSV, one row per line, in the user's number format (R-SET-6). XLSX is not planned.
- [x] Closing is blocked while there are open items, and each blocker can be accepted with a note. A closed event is read-only and shows its final balance; Settings reopens it (R-EVT-2, R-SET-7, R-SET-8).
- [x] Motion: every animation in the app, chosen on the `/design/motion` trial page, with a reduced-motion setting. See [design/motion.md](design/motion.md).
- [ ] **Parallel run** with the Excel sheet at the next event.

**Exit:** a real event is settled with Lancelot and its final balance matches Excel to the cent. Tag `v0.1.0`.

---

## Phase 2

### M6 · Budget and forecast

Built in three steps, each in its own pull request.

**M6a · Budget core (Rust only)**

- [x] Budget lines on an income or expense category, as an amount or as quantity × unit price, with a "complete" flag. Transaction lines link to a budget line of their own category; deleting a budget line leaves its actuals unbudgeted. Covers R-BUD-1 and R-BUD-2.
- [x] Plan vs actual per line, per category and in total, in the snapshot. Forecast rule: complete lines use the actual, otherwise the cautious one of estimate and actual (larger for expenses, smaller for income), plus unbudgeted actuals. Covers R-BUD-3, R-BUD-4 and R-BUD-5. Per cost centre came with M9.
- [x] Property tests: actuals add up to the ledger's totals, category figures add up from their lines, and budget lines never change a balance.

**M6b · Budget screen**

- [ ] **Design the Budget screen** in `UI.fig`.
- [x] The Budget page (⌘5 since M7b): forecast, planned result and result so far; estimate, actual and forecast per line and category, with what is not budgeted. A panel adds, edits and deletes budget lines and lists the receipts linked to each. Drafted before the design, following the People page.
- [x] Linking: each category line in the entry panel picks a budget line of its category, and keeps it when the receipt is edited.
- [ ] Match the page to the design once it is in `UI.fig`.

**M6c · Forecast in the Overview and the report**

- [x] The forecast on the Overview, with the planned result, leading to the Budget page. Hidden without a budget and once the event is closed.
- [x] Plan vs actual in the report: estimate, actual and forecast per category, with what is not budgeted, and the result (R-SET-2).

### M7 · Ticket import

Built in three steps, each in its own pull request.

**M7a · Ticket core (Rust only)**

- [x] Designs in `UI.fig`: the Import panel (columns read, snapshot date, an unknown tier to map, the changes and the rows not read), Edit tier, Manual sale and the empty Tickets page.
- [x] Tiers with price, fee in basis points (below the price), quota and aliases, unique across tiers. Covers R-TKT-1, R-TKT-2 and R-TKT-10.
- [x] Import `.ods` and `.xlsx` with `calamine`, and `.csv` with `csv`:
  - [x] Configurable columns, saved in the event.
  - [x] **Only those columns are read.**
  - [x] Unknown tiers block the import.
  - [x] Rows that cannot be read are listed.
  - Covers R-TKT-3, R-TKT-4, R-TKT-17 and R-TKT-18.
- [x] Re-import with a change preview per tier and per order. Every import is kept as a snapshot: file name, hash, date (changeable) and its tickets per (order ID, tier); the file itself is not stored, and order IDs stay out of the change log. Covers R-TKT-5 and the data for R-TKT-8.
- [x] Generated income per tier from the latest import, with a fee line in `Ticket fees`, marked as generated and read-only (R-TKT-6). Manual sales to a person (R-TKT-7). A file whose ticket income differs from its tickets does not open.
- [x] Fixtures: a series of `.ods` sheets with additions, a removal, a tier change and an unknown tier, plus `.xlsx` and `.csv` (`scripts/ticket-fixtures.py`).

**M7b · Tickets screen**

- [x] The Tickets page (⌘4, between People and Budget as in `UI.fig`): tiers with price, fee, sold vs quota and gross, manual sales per tier, ticket fees and ticket income. The tier, import and manual sale panels; the header line opens the earlier imports, whose snapshot dates can be changed.
- [x] Shell commands for tiers, the sheet's columns, preview and import, the import date and manual sales. The import passes the preview's SHA-256, so a file changed since it was read is not imported unseen. A file dialog and a drop target on the page for `.ods`, `.xlsx` and `.csv`.
- [x] Generated income is read-only in the ledger, tagged `TICKETS`, and leads to its tier or manual sale on the Tickets page.
- [x] The mock backend imitates the ticket commands with three made-up sheets (an unknown tier, a removed order, rows not read), so the screen can be clicked through in a browser.

**M7c · Conflicts**

- [x] Conflicts from removals between imports, by (order ID, tier): an order is short when it has fewer tickets than it ever had, so tickets that return close the gap and a growing tier hides nothing. Only the resolutions are stored.
- [x] Conflict cards on top of the Tickets page, grouped by order, with a badge in the navigation and an open item that blocks closing. Until resolved, the tickets are not sold, and the totals say so.
- [x] Refund 0–100 %, match or correction, wholly or in part, booked to `Ticket refunds` as generated, read-only negative income. Fees stay; a person's refund comes out of what they hold.
- [x] Manual cancellations on an order or a manual sale, in a panel listed under the tiers. A conflict on the same order and tier offers them for confirming. Deleting a resolution opens the conflict again.
- [x] Choice of refund payer: whoever received the money, or any account or person.
- [x] Property test: whatever the sheets and resolutions, the books balance, the figures match the ledger and a file loads back the same.
- Covers R-TKT-12 to R-TKT-16.

### M8 · Ticket statistics and profitability

Built in three steps, each in its own pull request.

**M8a · Statistics core**

- [x] Imports count in date order: the one with the latest date is the current sheet, and an older file only adds to the curve. The preview takes the snapshot date, counts from the import before it and says whether it becomes current.
- [x] A single "sales over time" source (`sales.rs`), fed by import snapshots now and able to switch to a sale date column later (R-TKT-8). One point per import, plus a last one for manual sales and refunds dated later, so the curve ends on the current figures.
- [x] Places against the quota: a cancelled ticket frees its place, whatever the refund, and stays sold for the money. The Tickets page shows places against the quota.
- [x] Projected result and break-even, overall and per tier, with the places left (R-TKT-9). The projected result is the forecast, and the ticket categories forecast what is booked, so an estimate on them no longer counts the money twice.
- [x] Property tests: the curve ends on the current figures, whatever the sheets and their dates, and the break-even count is the smallest that closes the gap.

**M8b · Tickets page**

- [x] Designs in `UI.fig`: the Tickets frame and `Tickets / Statistics`, with revenue, cancellations, break-even per tier and the states of the block.
- [x] The break-even block beside the tiers, or under them with a panel open: the amount missing, about how many tickets close it, the places left when they fall short, and the count per tier. Without sales the estimate uses the average of the tiers; without tiers there is `+ Tier` (R-TKT-9).
- [x] Stacked bars per snapshot, by places against the quota with the cancelled on top, or by net income (R-TKT-8).

**M8c · Overview**

- [x] The ticket block on the Overview: places against the quota, leading to the Tickets page. The break-even stands under the forecast while there is a gap, with the places left when they fall short (R-TKT-9).

### M9 · Cost centres and Drinks

- [x] Cost centre tags on income and expense lines and on budget lines, with a P&L and plan vs actual per cost centre on the Budget page and in the report. Drinks exists in every event and cannot be deleted; a category can name the cost centre its new lines start in, and Drinks, Pfand and Drink donations start in Drinks. A line linked to a budget line has that line's cost centre, and follows it. Generated ticket income is not tagged. The transactions CSV gains a last column. Covers R-CC-1, R-CC-2, R-BUD-3 and R-SET-3.
- [x] Drinks page (⌘5, Budget moves to ⌘6): cost, donations and result of the cost centre, per-product commission records, the cost per drink and the price per Strich. The invoice check compares the Drinks-tagged expenses whose counterparty is the supplier, without Pfand, with what the records say; **Supplier invoice** opens that expense in the ledger, filled in from the records. A difference shows on the page and is not an open item. Covers R-DRK-1 to R-DRK-6.

### M10 · Reconciliation and templates

- [x] Reconciliation checkpoints per asset account, one per date, from a **Statement** panel on the Overview. The Verein account is checked on net movement, without opening counts (R-ACC-4, R-ISO-2), and every mismatched checkpoint is an open item, however later ones turn out (R-SET-5).
- [x] New event from a template: accounts, categories, cost centres, persons, budget lines, tiers, sheet columns and the drink price list, with no money and nothing marked complete. Copying persons and budget lines from another file into an open event, in Settings, never overwrites: what is already there is skipped, and a missing category or cost centre is created. The other file is only read. Covers R-EVT-3 and R-ISO-4.

**Phase 2 exit:** the next event runs with ticket imports, a budget and a Drinks P&L, and Excel is no longer needed. Tag `v0.2.0`.

---

## Refactoring

From the [architecture audit](https://claude.ai/artifact/7rTXtHaDHY5K9g423LZVBm) of 2026-10-30, made at commit `071e403`; F-numbers refer to its findings. The stages run in order and have no deadline. `0001_initial.sql` was edited in place until Stage 6 froze the format, and Stage 7 unfroze it until the first real event is entered. Balancing, integer cents and derived balances (R-COR-1 to R-COR-6) are never traded for flexibility.

### Stage 0 · Docs

- [x] This section, and [future.md](future.md) for longer-term changes.
- [x] Trim the technical design to decisions and invariants, and correct what had drifted (F22). `UI.fig` stays private.
- [x] Remove the untracked `app/pnpm-workspace.yaml`: pnpm generated it, and its exclusions are no longer needed.

### Stage 1 · Dev bridge (F8)

The browser checks run against the real core, and the mock backend is gone.

- [x] `crates/lancelot-bridge`, development only and bound to `127.0.0.1`, exposes the real `Service`'s commands. `backend()` in `lib/api` calls it in a plain browser, and without it the browser shows how to start it. `Service` moved to `crates/lancelot-service`, so the bridge needs no Tauri.
- [x] The scenarios `start`, `example`, `empty`, `conflict` and `closed`, built on the core's example event, with a damaged and a locked file among the recent events.
- [x] Attachments served by the bridge, with real receipt files in `fixtures/receipts`. The dialogs' stand-ins in `lib/api/dev.ts`.
- [x] The bridge in `.claude/launch.json`, and every screen and `/design` checked through it.
- [x] `app/src/lib/api/mock/` deleted. The example event the UI tests use is now in `lib/api/fixtures`.
- [x] The service and bridge tests run on every push, which covers the shell tests of F14.

**Exit:** the UI runs in a browser against real SQLite, and nothing imports the mock. ✓

### Stage 2 · Protect files

- [x] Re-imports no longer break earlier statement checkpoints: ticket income in a checkpoint comes from the sheets known by its date, and the Statement panel shows that part (F1).
- [x] Generated ticket entries are regenerated on open. A difference is written with change log entries, and changed money is an integrity warning, so the file no longer opens as damaged (F2).
- [x] `delete_import`, refused only while a cancellation still needs its tickets and with a preview of what it changes, and `delete_account` for accounts and people nothing refers to, in the Imports panel, Settings and the person panel (F5).
- [x] Negative counts on bank accounts, with closing counts and checkpoints compared through one function. Every asset account now has Count on the Overview (F6, R-ISO-3).
- [x] Errors appear where they occur: payouts and profitability computed in `recompute`, a negative opening count rejected, a receipt about to be lost shown in the import preview, and `inconsistent` for a file that is intact but fails the rules (F13).
- [x] A property test over imports, checkpoints and deleting a sheet, ending with a reopened file (F14).

### Stage 3 · Remove code, no behaviour change

In this order, so the round-trip check is there for the `persist` rewrite.

- [x] Each write checks one round trip, `Book::from_parts(read::parts(tx))? == next`, in place of the quick check, and compares the generated entries as stored. The `Book` built during the check on open is reused (F9).
- [x] A public `Store::change` and one generic `Service` helper replace the forwarding methods. Multi-step operations such as `mark_payouts_paid` and `set_counts` move into `Book` (F7).
- [x] `PRAGMA defer_foreign_keys = ON` in `persist`, with each table handled in one place by a small helper (F10).
- [x] One function for the money of a tier and count, and one for a refund, used by `sync_tickets`, the views and the figures (F11).
- [x] One error helper, a shared form stylesheet and one session method (F20).

### Stage 4 · Decouple

- [x] `TxnKind::Refund`, so the ledger reads a kind, not an origin. This adds one value to the `txn.kind` CHECK. `Book::open_items()` combines the items of each feature. `ticket_columns` and `drink_settings` move from `Event` to `Book` and are logged on their own; the columns stay on the event row. `Ledger::compute` is split by figure (F15).
- [x] `TxnView` says what the UI inferred: who, the flow, whether a receipt is needed and the `Entry` for the form. An opening count opens read-only and leads to the Overview, where it was already edited, because saving it as a transfer always failed. The `ticket_share` command previews fee and refund amounts by the core's rule (F12).

### Stage 5 · Open the flows

The remaining schema changes of the refactoring landed here, as one batch.

- [x] **Any account to any account (F3).** The Transfer form has free From and To fields over assets, people and categories. So A → B: 50 € means A is owed 50 € and B owes 50 €, and B's later 50 € expense nets B to zero. The opening account stays reserved for opening entries, and generated ticket entries stay read-only. Expense and Income stay as forms with receipts and splits. `TxnKind` is only a label; stored data is checked for balance, at least two non-zero lines, known accounts and the opening rule. `flow` now comes from what the lines do to the result.
- [x] **Roles go (F4).** A ticket account setting, a destination (asset or person) on manual sales and a net-movement flag on asset accounts replace `Role::Verein`, `CashBox` and `Opening`. The ticket category roles stay.
- [x] **Optional tickets.** A per-event switch in Settings. Off hides the page and the Overview block, and removes the ticket categories, keeping one in use as an ordinary category; it is refused while tiers exist. The ticket account is set next to the column mapping and is needed only to import. Templates copy both.
- [x] **Optional drinks (F16).** `DrinkSettings` names the cost centre and the Pfand category, each optional, replacing `CostCentreRole` and `Role::Pfand`. Both are created by default and can be deleted; the cost centre stays while it holds records.
- [x] **Cost centres in the sidebar.** The pages, a separator, a "Cost centres" label and one item per centre, also on ⌘1–9. Each centre's page is built from existing components: income, expenses, result and tagged lines (R-CC-1). The drinks centre adds its products and the invoice check.
- [x] **Fields that do nothing (F17).** The sale date input is hidden until it is read, `Line.memo` is gone, the entry form has a notes input (R-TX-1), and M11 is planned as a plain CSV import.
- [x] **Reopening** keeps the closing notes and prefills those that still match an open item (F18).
- [x] **`sealed` and its triggers are gone (F9).** Balance stays checked at construction, before each commit and on every load. The column-level SQL CHECKs stay.
- [x] R-COR-2, R-TKT-3, R-TKT-6, R-TKT-7, R-TKT-15, R-TKT-19, R-ISO-3, R-ACC-4, R-CC-1, R-CC-2, R-TX-3, R-SET-7 and R-STR-1 amended, and the technical design updated.

### Stage 6 · Freeze the format

- [x] `fixtures/events/sample-v1.lancelot` is checked in, with tickets, imports, cancellations, checkpoints, drinks, attachments and a closed event: rows in every table and every transaction kind. A test keeps it opening with its figures written out (F14). From here on every schema change is a migration, and a test fails if `0001_initial.sql` is edited.

### Stage 7 · Built-in plugins

- [x] The schema is unfrozen until the first real event is entered, so `0001_initial.sql` is edited directly and the sample regenerated. The sample's money figures stay as they were written out.
- [x] `plugin_record` and `Book::apply_plugin_changes`: records in the event file, with change log and read-back, copied by templates by scope. The core no longer knows about drinks (technical design §10).
- [x] `lib/plugins`: `PluginContext`, the host, the registry, and sections on cost centre pages and in Settings. A test keeps plugins behind the context.
- [x] Data sources: a form in Settings, details on this computer, the token in the keychain and the request in Rust. Drinks and the Ticket API plugin pull from them.
- [x] Ticket sheet reading in `lib/plugins/ticket-sheet`. The core takes a `TicketBatch` from any source and keeps tiers, conflicts and income; the column names became the plugin's record.
- [x] Drinks in `lib/plugins/drinks`, on any cost centre, with donation prices, left-over and logged counts, expected against collected, and an invoice proposed through the normal form. A new event starts without drinks.

### After the next event

- [ ] Decide whether a small change history view is worth having. If not, reduce R-TX-6. The log keeps being written until then (F21).

---

## Phase 3

- **M11 · Drink sales import.** The pull from a data source is done, with a preview, name matching and only per-product totals (R-DRK-6). Importing from a file is left, and needs a sample of the drinks app's export.
- **M12 · SEPA.** Export the payout list as pain.001 XML (R-PER-6).
- **M13 · Polished reports.** A typeset settlement PDF that follows the design system.
- **M14 · Receipt OCR** (optional): pre-fill amount, date and counterparty from an attached receipt. It must run locally.
- **User manual.** How the event's money is modelled (accounts, people, counts) and how to run an event from opening count to closing, for treasurers who did not build it.

---

## Open decisions and dependencies

| What                                          | Needed by    | Owner  |
| --------------------------------------------- | ------------ | ------ |
| Date of the next event: not soon, no deadline | M5           | you    |
| Apple Developer account for signing           | after v0.1.0 | you    |
| Sale date column in the ticket sheet          | M8           | Verein |
| A real (anonymised) ticket sheet as a fixture | M7           | you    |

## Next step

The [refactoring stages](#refactoring) are done, apart from the decision that waits for the next event. First try the data sources in the desktop app with a real provider and a token in the keychain, which hasn't been done yet. Then run the next event in parallel with Excel: it is both the M5 parallel run and the Phase 2 exit (`v0.2.0`), and entering it freezes the file format again. The Budget screen still needs its design (M6b). Importing drink sales from a file (M11) follows once there is a sample of the export. Longer-term changes are in [future.md](future.md).
