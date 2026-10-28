# Lancelot — Roadmap

Status: v0.3. M0 and M1 are done. It builds on [requirements v0.7](requirements.md) and the [design notes](design/README.md). Requirement IDs (R-…) are listed for each milestone, so you can trace every one.

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
- [ ] **Design the start screen, the new-event panel and the Settings screen** in `UI.fig`, plus the integrity banner and the "already open" and "damaged" states.
- [ ] Svelte components from `docs/design/README.md`, on a development-only `/design` page that mirrors the System frame:
  - [ ] Button, filter chip, segmented control, text field, select, date field
  - [ ] **Amount input with live preview** (calls the core parser)
  - [ ] Balance cell, receipt indicator, table with day groups, right-hand panel, stacked bar
- [ ] A start screen with recent events, plus New and Open, using native file dialogs.
- [ ] The event window: sidebar with the event, its state and dates, navigation with counts and attention badges, and the file name.
- [ ] Settings: number/date format (R-NF-6), backup count and the backup list with restore, event details and the default reimbursement account, the file and its integrity check.
- [ ] A keyboard model: ⌘O, ⇧⌘N, ⌘, , ⌘W, ⌘1–3 and Esc (R-NF-7).
- [ ] An integrity banner with "Check again" (R-COR-5).
- [ ] A development-only mock backend, so the screens can be checked in a browser.

**Exit:** you can create, open and rename an event and switch the number format. The amount input behaves as it does in the System frame.

### M3 · Ledger

The first slice the treasurer can use: entering the stack of receipts.

- [ ] Ledger table: grouped by day or by category with totals (R-TX-7), filter chips (type, account, person, category, missing receipt), and search.
- [ ] Entry panel with the Expense / Income / Transfer / Settle switch, split lines with a running remainder, and the payer's live balance.
- [ ] Editing and deleting, with every change in the change log.
- [ ] Attachments: add, preview (image and PDF), open the original, and export. Drag a file onto the window to start a new transaction with the file attached. Covers R-ATT-3 and R-ATT-5.
- [ ] HEIC preview: convert in Rust, or fall back to "open externally". To be decided in M0.
- [ ] A "missing receipts" filter (R-ATT-4).
- [ ] Opening and closing count for the cash box, with the discrepancy (R-ISO-3).

**Exit:** the 30–40 receipts of a past event can be entered with the keyboard alone in under 30 minutes.

### M4 · People and payouts

- [ ] Person records with IBAN validation (checksum and format) and an IBAN warning in the list. Covers R-PER-1 and R-PER-2.
- [ ] People table: advanced, holds, settled and balance. A person panel with the running balance and a drill-down to transactions and receipts (R-PER-3).
- [ ] Settle action, pre-filled with the net balance. Partial payments are allowed (R-TX-4).
- [ ] Payout list export as CSV and PDF, with the optional itemised appendix (R-PER-4).
- [ ] Repayment list (R-PER-7).
- [ ] "Mark payout list as paid": record all of its reimbursements in one action (R-PER-5).

**Exit:** for the fixture event, the payout list says Anna 212,40 € and Malte 110,00 €, and the repayment list says Bob 180,00 €. One click settles both payouts.

### M5 · Settlement and closing (MVP release)

- [ ] Overview screen, MVP version: result, in/out by category, "where the money is" and open items. The forecast and ticket blocks are hidden until Phase 2.
- [ ] Settlement report: income and expenses by category, result, final balance per asset account, and open items. Covers R-SET-1, R-SET-4 and R-SET-5.
- [ ] Exports: PDF, plus CSV/XLSX of the raw transactions (R-SET-6).
- [ ] Closing is blocked while there are open items, and each blocker can be overridden with a note. After closing, the final balance stays visible (R-SET-7, R-SET-8).
- [ ] Signed macOS build, or at least notarised. This needs an Apple Developer account, and the open question is whose.
- [ ] **Parallel run** with the Excel sheet at the next event.

**Exit:** a real event is settled with Lancelot and its final balance matches Excel to the cent. Tag `v0.1.0`.

---

## Phase 2

### M6 · Budget and forecast
- [ ] **Design the Budget screen** in `UI.fig`.
- [ ] Budget lines, as an amount or as quantity × unit price, with links from actual lines. Covers R-BUD-1 and R-BUD-2.
- [ ] Plan vs actual per line, category, cost centre and in total. Forecast rule: complete lines use the actual, otherwise the larger of estimate and actual, plus unbudgeted actuals. Covers R-BUD-3, R-BUD-4 and R-BUD-5.
- [ ] Forecast block on the Overview, and plan vs actual in the report (R-SET-2).

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

---

## Open decisions and dependencies

| What | Needed by | Owner |
|---|---|---|
| Date of the next event: sets the MVP deadline | M0 | you |
| Apple Developer account for signing | M5 | you |
| Sale date column in the ticket sheet | M8 | Verein |
| A real (anonymised) ticket sheet as a fixture | M7 | you |

## Next step

M2b: the screens and components, starting with the designs in `UI.fig`.
