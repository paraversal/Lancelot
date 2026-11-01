# Lancelot — Roadmap

Status: v0.11. Phases 1 and 2 are built, and most of what's left of them needs a real event. The roadmap builds on [requirements v0.13](requirements.md) and the [design notes](design/README.md), and what exists today is listed in [features](features.md). Finished work is kept to a summary here; the full checklists are in git, with `git show bb9e4a8:docs/roadmap.md`.

## Next step

First try the data sources in the desktop app with a real provider and a token in the keychain, which hasn't been done yet. Then run the next event in parallel with Excel: it is both the M5 parallel run and the Phase 2 exit (`v0.2.0`), and entering it freezes the file format again. The Budget screen still needs its design (M6). Importing drink sales from a file (M11) follows once there is a sample of the export. Longer-term changes are in [future.md](future.md).

## Still open

- [ ] **Parallel run** with the Excel sheet at the next event (M5). A real event is settled with Lancelot and its final balance matches Excel to the cent. Tag `v0.1.0`.
- [ ] **Phase 2 exit:** the next event runs with ticket imports, a budget and a Drinks P&L, and Excel is no longer needed. Tag `v0.2.0`.
- [ ] **Design the Budget screen** in `UI.fig`, and match the page to it (M6).
- [ ] Try the data sources with a real provider.
- [ ] Enter a stack of receipts by keyboard: the 30–40 receipts of a past event in under 30 minutes (M3's exit test, yours to try).
- [ ] Click through the native dialogs and menu in the app (M2).

## Phase 3

- **M11 · Drink sales import.** The pull from a data source is done, with a preview, name matching and only per-product totals (R-DRK-6). Importing from a file is left, and needs a sample of the drinks app's export.
- **M12 · SEPA.** Export the payout list as pain.001 XML (R-PER-6).
- **M13 · Polished reports.** A typeset settlement PDF that follows the design system.
- **User manual.** How the event's money is modelled (accounts, people, counts) and how to run an event from opening count to closing, for treasurers who did not build it.

## Open decisions and dependencies

| What                                          | Needed by    | Owner  |
| --------------------------------------------- | ------------ | ------ |
| Date of the next event: not soon, no deadline | M5           | you    |
| Apple Developer account for signing           | after v0.1.0 | you    |
| Sale date column in the ticket sheet          | M8           | Verein |
| A real (anonymised) ticket sheet as a fixture | M7           | you    |

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

## Done

### Phase 1 — MVP

- **M0 · Technical design and scaffold.** The technical design, the workspace with Tauri 2 and SvelteKit, CI and the locally bundled font (R-NF-3).
- **M1 · Ledger core.** Money, amount parsing, IBANs, accounts, transactions, derived balances, closing, SQLite storage, the change log, backups, counts and the integrity check, with the example event reaching **+2.027,50 €** after a reopen (R-COR-1 to R-COR-5, R-NF-5, R-NF-6, R-NF-8, R-PER-2, R-ACC-1 to R-ACC-3, R-TX-1 to R-TX-6, R-EVT-1, R-EVT-2, R-EVT-4, R-ATT-1, R-ATT-2, R-ISO-3, R-SET-5, R-SET-7).
- **M2 · App shell and design system.** Dates, the event snapshot, generated TypeScript types, settings, the start screen, the event window, the components and the keyboard model (R-NF-6, R-NF-7, R-COR-5).
- **M3 · Ledger.** The ledger table, the entry panel, editing and deleting, and receipts with preview and drag-and-drop (R-TX-7, R-ATT-1 to R-ATT-5).
- **M4 · People and payouts.** The People screen, and the payout and repayment lists with "Mark as paid" (R-PER-1 to R-PER-5, R-PER-7, R-TX-4).
- **M5 · Settlement and closing.** Bank and cash accounts, counts, the Overview, the settlement report, the transactions CSV, closing with blockers, and motion (R-ACC-1, R-ISO-3, R-SET-1, R-SET-4 to R-SET-8, R-EVT-2). Only the parallel run is left.

### Phase 2

- **M6 · Budget and forecast.** Budget lines, plan vs actual, the forecast, the Budget page and the budget in the Overview and report (R-BUD-1 to R-BUD-5, R-SET-2). Only the design is left.
- **M7 · Ticket import.** Tiers, sheet imports with a change preview and snapshots, generated income, manual sales, conflicts and refunds (R-TKT-1 to R-TKT-7, R-TKT-10, R-TKT-12 to R-TKT-18).
- **M8 · Ticket statistics and profitability.** Sales over time, places against the quota and break-even, on the Tickets page and the Overview (R-TKT-8, R-TKT-9).
- **M9 · Cost centres and Drinks.** Cost centre tags with a P&L and plan vs actual, and the Drinks page, which became a plugin in Stage 7 (R-CC-1, R-CC-2, R-BUD-3, R-SET-3, R-DRK-1 to R-DRK-6).
- **M10 · Reconciliation and templates.** Statement checkpoints, new events from a template, and copying from another event (R-ACC-4, R-ISO-2, R-ISO-4, R-SET-5, R-EVT-3).

### Refactoring

From the [architecture audit](https://claude.ai/artifact/7rTXtHaDHY5K9g423LZVBm) of 2026-10-30, made at commit `071e403`; F-numbers refer to its findings. Balancing, integer cents and derived balances (R-COR-1 to R-COR-6) are never traded for flexibility.

- **Stage 0 · Docs.** `future.md`, and a technical design trimmed to decisions and invariants (F22).
- **Stage 1 · Dev bridge.** Browser checks run against the real core through `crates/lancelot-bridge`, and the mock backend is gone (F8).
- **Stage 2 · Protect files.** Re-imports keep earlier checkpoints, generated entries are regenerated on open, imports and unused accounts can be deleted, and errors appear where they occur (F1, F2, F5, F6, F13, F14).
- **Stage 3 · Remove code.** One round-trip check on every write, and fewer forwarding methods and duplicated helpers, with no change in behaviour (F7, F9, F10, F11, F20).
- **Stage 4 · Decouple.** The ledger reads a transaction's kind and not its origin, and `TxnView` says what the UI used to infer (F12, F15).
- **Stage 5 · Open the flows.** Transfers between any accounts, no more roles, optional tickets and drinks, cost centres in the sidebar, and no fields that do nothing (F3, F4, F9, F16, F17, F18, R-TKT-19, R-STR-1).
- **Stage 6 · Freeze the format.** The checked-in sample event, with a test that keeps it opening (F14).
- **Stage 7 · Built-in plugins.** Plugin records and context, data sources, and ticket sheets and drinks moved into plugins, with the schema unfrozen until the first real event is entered.
