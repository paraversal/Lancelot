# Lancelot — Roadmap

Status: v0.9. M0 to M4 are done, and M5 is done except the parallel run (M3's exit test, a stack of receipts by keyboard, is yours to try). M6 is in progress. It builds on [requirements v0.9](requirements.md) and the [design notes](design/README.md). Requirement IDs (R-…) are listed for each milestone, so you can trace every one.

## How we work

- **Core first, UI second.** Every rule about money lives in `lancelot-core` and is tested there before any screen uses it (R-COR-6). The UI only formats values and sends commands.
- **Vertical slices.** From M3 on, each milestone ends with something the treasurer can actually use, not with a finished layer.
- **Definition of done** for each milestone:
  - `cargo test` passes, including the property tests.
  - `cargo clippy -D warnings` passes, and `svelte-check` and the UI tests pass.
  - The integrity check passes on the milestone's test event.
  - The screens match `UI.fig`. Deviations are noted in `docs/design/README.md`.
- **A real event as the yardstick.** The MVP must be usable for the next event, run **in parallel with the Excel sheet**. The result is a release only when both arrive at the same final balance.

## Repository layout (target)

```
Cargo.toml                 workspace
crates/lancelot-core/      domain, storage, import, reports (no Tauri dependency)
app/                       SvelteKit SPA (adapter-static, TypeScript)
app/src-tauri/             Tauri 2 shell: commands, dialogs, drag-and-drop
docs/                      requirements, roadmap, design, technical design
fixtures/                  sample .lancelot events, ticket sheets (.ods), Strichliste CSVs
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
  - **PDF export:** Typst as a library.
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
- [x] Plan vs actual per line, per category and in total, in the snapshot. Forecast rule: complete lines use the actual, otherwise the cautious one of estimate and actual (larger for expenses, smaller for income), plus unbudgeted actuals. Covers R-BUD-3, R-BUD-4 and R-BUD-5. Per cost centre follows with cost centres in M9.
- [x] Property tests: actuals add up to the ledger's totals, category figures add up from their lines, and budget lines never change a balance.

**M6b · Budget screen**

- [ ] **Design the Budget screen** in `UI.fig`.
- [x] The Budget page (⌘4): forecast, planned result and result so far; estimate, actual and forecast per line and category, with what is not budgeted. A panel adds, edits and deletes budget lines and lists the receipts linked to each. Drafted before the design, following the People page.
- [x] Linking: each category line in the entry panel picks a budget line of its category, and keeps it when the receipt is edited.
- [ ] Match the page to the design once it is in `UI.fig`.

**M6c · Forecast in the Overview and the report**

- [x] The forecast on the Overview, with the planned result, leading to the Budget page. Hidden without a budget and once the event is closed.
- [x] Plan vs actual in the report: estimate, actual and forecast per category, with what is not budgeted, and the result (R-SET-2).

### M7 · Ticket import

- [ ] Tiers with price, fee in basis points, quota and aliases. Covers R-TKT-1, R-TKT-2 and R-TKT-10.
- [ ] Import `.ods`, `.xlsx` and `.csv` with `calamine`:
  - [ ] Configurable columns.
  - [ ] **Only those columns are read.**
  - [ ] Unknown tiers block the import.
  - [ ] Rows that cannot be parsed are listed.
  - Covers R-TKT-3, R-TKT-4, R-TKT-17 and R-TKT-18.
- [ ] Re-import with a change preview. Store the file name, hash and rows; the file itself is not stored. Covers R-TKT-5.
- [ ] Generated income per tier with a fee line, marked as imported and read-only (R-TKT-6). Manual sales to a person (R-TKT-7).
- [ ] Conflicts:
  - [ ] Change detection by (order ID, tier).
  - [ ] Conflicts view with a navigation badge.
  - [ ] Refund 0–100 %, match or correction.
  - [ ] Manual cancellations with automatic matching.
  - [ ] Choice of refund payer.
  - Covers R-TKT-12 to R-TKT-16.
- [ ] Fixtures: a series of `.ods` sheets with additions, removals and an unknown tier.

### M8 · Ticket statistics and profitability

- [ ] A single "sales over time" source. It is fed by import snapshots now and can switch to a sale date column later (R-TKT-8).
- [ ] Stacked bars per snapshot, sold vs quota, revenue and cancellations.
- [ ] Projected result and break-even, both overall and per tier (R-TKT-9). The ticket block appears on the Overview.

### M9 · Cost centres and Drinks

- [ ] Cost centre tags on lines and budget lines, a P&L per cost centre, and Drinks as the default cost centre. Covers R-CC-1, R-CC-2 and R-SET-3.
- [ ] Drinks screen: cost (drinks plus Pfand), donations and result, per-product commission records with an invoice check, and the price per Strich. Covers R-DRK-1 to R-DRK-6.

### M10 · Reconciliation and templates

- [ ] Reconciliation checkpoints per asset account. The Verein account is checked on net movement (R-ACC-4), and mismatches become open items.
- [ ] New event from a template, and copying persons and budget from another file. Covers R-EVT-3 and R-ISO-4.

**Phase 2 exit:** the next event runs with ticket imports, a budget and a Drinks P&L, and Excel is no longer needed. Tag `v0.2.0`.

---

## Phase 3

- **M11 · Strichliste.** A `StrichlisteProvider` trait, a CSV reference provider, tally display with expected vs actual revenue and unpaid amounts, and a per-event toggle (R-STR-1 to R-STR-5).
- **M12 · SEPA.** Export the payout list as pain.001 XML (R-PER-6).
- **M13 · Polished reports.** A typeset settlement PDF that follows the design system.
- **M14 · Receipt OCR** (optional): pre-fill amount, date and counterparty from an attached receipt. It must run locally.
- **User manual.** How the event's money is modelled (accounts, people, counts) and how to run an event from opening count to closing, for treasurers who did not build it.

---

## Open decisions and dependencies

| What                                          | Needed by    | Owner  |
| --------------------------------------------- | ------------ | ------ |
| Date of the next event: sets the MVP deadline | M0           | you    |
| Apple Developer account for signing           | after v0.1.0 | you    |
| Sale date column in the ticket sheet          | M8           | Verein |
| A real (anonymised) ticket sheet as a fixture | M7           | you    |

## Next step

M6b: design the Budget screen in `UI.fig` (with the Overview's forecast figure) and match the drafted page to it. Then M7: ticket import. The M5 parallel run happens at the next event, alongside it.
