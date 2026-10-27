# Lancelot — Roadmap

Status: v0.2. M0 is done. It builds on [requirements v0.7](requirements.md) and the [design notes](design/README.md). Requirement IDs (R-…) are listed for each milestone, so you can trace every one.

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

**Exit:** `npm run tauri dev` opens a window that shows the empty sidebar in the right font and colours, and CI passes.

### M1 · Ledger core (Rust only, no UI)

This is the heart of the app. It gets the most tests.

- [ ] `Cents(i64)` with checked arithmetic, `allocate(total, n | weights)` with remainder cents going to the first lines, and `percent_bp_half_up`. Covers R-COR-1 and R-COR-3.
- [ ] Amount parser and formatter. Both formats, rejecting invalid grouping, flagging the ambiguous three-digit case, and handling the sign rules. Covers R-NF-8 and R-NF-6.
- [ ] Accounts by kind: asset, person, income, expense, opening balance. Default accounts and categories. Covers R-ACC-1 and R-ACC-2.
- [ ] Transactions with split lines, balanced by construction. Covers R-TX-1 and R-TX-2.
- [ ] Typed builders: expense, income, transfer, reimbursement/repayment. They map the form inputs to lines, following the flow table in §4.1. Covers R-TX-3.
- [ ] Derived balances: per account, per person net balance and status, and totals per category. Covers R-ACC-3, R-TX-4, R-TX-5 and R-COR-4.
- [ ] Event lifecycle Planning → Running → Settling → Closed. Closed is read-only, and reopening is logged. Covers R-EVT-1, R-EVT-2 and R-EVT-4.
- [ ] Change log with before and after values for every create, edit and delete. Covers R-TX-6.
- [ ] SQLite storage:
  - [ ] Create and open a `.lancelot` file.
  - [ ] Atomic writes.
  - [ ] Attachments as BLOBs with SHA-256 (R-ATT-1, R-ATT-2).
  - [ ] Schema version and migrations.
  - [ ] Backups of the last N versions (R-NF-5).
- [ ] Integrity check on open, on save and on demand. Covers R-COR-5.
- [ ] Tests:
  - [ ] Unit tests for each requirement.
  - [ ] Property tests with `proptest`, for example: all balances sum to 0, splits sum to the total, parse(format(x)) = x, and the integrity check holds after any valid sequence of operations.
  - [ ] A fixture event that rebuilds the mock data in `UI.fig` and reaches **+2.027,50 €**.

**Exit:** a CLI or test can create the mock event, save it, reopen it and print correct balances. The core has no TODOs in the money path.

### M2 · App shell and design system in code

- [ ] Svelte components from `docs/design/README.md`:
  - [ ] Button
  - [ ] Filter chip
  - [ ] Segmented control
  - [ ] **Amount input with live preview** (calls the core parser)
  - [ ] Balance cell
  - [ ] Receipt indicator
  - [ ] Table with day groups
  - [ ] Right-hand panel
  - [ ] Stacked bar
- [ ] A start screen with recent events, plus New and Open, using native file dialogs.
- [ ] The event window: sidebar navigation with counts and attention badges.
- [ ] **Design the Settings screen** in `UI.fig` first, then build it: number/date format (R-NF-6), backup count, and event settings (name, dates, default reimbursement account).
- [ ] A keyboard model: `N` new, `⌘↵` save, Esc closes the panel, and arrow keys move through tables (R-NF-7).
- [ ] An integrity-failure banner (R-COR-5).

**Exit:** you can create, open and rename an event and switch the number format. The amount input behaves as it does in the System frame.

### M3 · Ledger

The first slice the treasurer can use: entering the stack of receipts.

- [ ] Ledger table: grouped by day, filter chips (type, account, person, category, missing receipt), and search.
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

M1: the ledger core in `crates/lancelot-core`, starting with `money` and `amount`.
