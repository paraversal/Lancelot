# Lancelot — Technical design

Status: v0.3. It implements [requirements v0.13](requirements.md) and follows the [roadmap](roadmap.md). It records decisions and invariants that the code does not explain by itself. For module layout, signatures, the commands and the routes, read the code.

## 1. Components

```
┌────────────── app/ (SvelteKit SPA, Svelte 5, TypeScript) ──────────────┐
│ screens · components · plugins · formatting · keyboard model           │
└───────────────▲───────────────────────────────┬────────────────────────┘
                │ EventSnapshot (JSON)          │ commands (invoke)
┌───────────────┴───────────── app/src-tauri/ ──▼────────────────────────┐
│ Tauri 2 shell: command handlers, file dialogs, drag-and-drop,          │
│ attachment protocol. crates/lancelot-service: app settings, open event │
└───────────────▲───────────────────────────────┬────────────────────────┘
                │                               │
┌───────────────┴──────────── crates/lancelot-core/ ─▼───────────────────┐
│ money · amount · model · entry · ledger · store · integrity · export   │
│ no Tauri dependency, usable from tests and a CLI                       │
└────────────────────────────────────────────────────────────────────────┘
                                │
                         event.lancelot (SQLite)
```

**Rules:**

- No arithmetic on money in TypeScript. The UI receives every figure from the core, already computed, as integer cents. It only _formats_ them. The one exception is a plugin's own figures, which never affect the ledger (§10).
- One window holds one open event. App-wide settings live outside the event files (§6.4).

## 2. Money

### 2.1 Representation

- `Cents(i64)` is a newtype with `checked_add`, `checked_sub` and `checked_mul(i64)`. An overflow is a `DomainError::Overflow`, never a wrap.
- It crosses the IPC boundary as a JSON number. All plausible values are far below 2⁵³, and the core rejects any amount above `MAX_AMOUNT = 10¹²` cents (10 billion of the main unit).

### 2.2 Rounding and splitting (R-COR-3)

**`allocate(total, n)`** splits `total` into `n` parts:

- `base = total / n`, truncated towards zero.
- `r = |total − base·n|`.
- The first `r` parts get one extra cent, with the sign of `total`.

Examples:

- `allocate(10,00 €, 3)` → `3,34 · 3,33 · 3,33`
- `allocate(−0,05 €, 2)` → `−0,03 · −0,02`

**`percent_bp(amount, bp)`** computes `amount × bp / 10 000` and rounds half away from zero. For the non-negative amounts where it is used, that is the same as rounding half-up.

- In integers: `(amount·bp + 5 000) / 10 000` for `amount ≥ 0`, and symmetrical for negative amounts.
- Examples:
  - `percent_bp(25,00 €, 250)` → `0,63 €` (exactly 0,625)
  - `percent_bp(32,00 €, 250)` → `0,80 €`

**Per-ticket rule** (R-TKT-10, R-TKT-15): the fee and the refund are computed for **one ticket**, then multiplied by the count.

- Example: fee for 128 × Regular at 32 € with a 2.5 % fee = `128 × 0,80 €` = `102,40 €`.
- Example: refund for 3 × Early Bird at 25 € with a 50 % refund = `3 × percent_bp(25,00 €, 5000)` = `3 × 12,50 €` = `37,50 €`.
- `ticket_share` is the one place for this rule. The forms preview through the command of the same name, and the UI has no rounding of its own.

**Quantity × unit price** (budget lines, drink products) is an exact multiplication, so no rounding is needed.

### 2.3 Amount input (R-NF-8)

The parser works in these steps:

1. Strip spaces and `€`. Accept one leading `-`, `−` or `+`. A sign anywhere else is `MisplacedSign`.
2. Find the decimal separator: the last `.` or `,` that is followed by 1–2 digits and nothing else.
3. Treat every other separator as a thousands separator. It must be the other character, the groups must be 3 digits, and the first group 1–3 digits.
4. Four or more digits after the last separator, or three digits after a last separator that differs from the others (`1.234,567`), are `TooManyDecimals`.
5. A single separator followed by exactly 3 digits is _ambiguous_ and read according to `fmt`:
   - If it is the thousands separator of `fmt` (`1.234` in De, `1,234` in En), the amount is accepted and `ambiguous = true`, so the preview is shown in `attention`.
   - If it is the decimal separator of `fmt` (`1,234` in De, `1.234` in En), it means three decimals and is rejected as `TooManyDecimals`. A German user who types `1,234` almost certainly meant `1,23` or `12,34`, and reading it as a thousand euros would be a 1000× error.

**Sums.** `AmountField` calls `parse_sum` (`amount_sum`), which splits the input at every sign after the first and reads each term with the parser above. One sign at the end is ignored, an empty term is `MissingTerm`, and the negative check applies to the total. It returns the total with the number of terms; the field keeps the typed text when there is more than one, and only the total is saved. Everything that reads a value from elsewhere, such as a file cell or a plugin's `parseAmount`, stays with `parse_amount`, so `10-12` in a spreadsheet is an error and not `-2`. The test cases live in `fixtures/amount-sum.json`.

**Where the parser runs:** in Rust, behind the synchronous command `amount_parse`. A round trip takes well under a millisecond, so the live preview needs no second implementation in TypeScript. The test cases live in `fixtures/amount-input.json`, which holds input, format, the expected result and the ambiguity flag. They are read by the Rust tests, and later also by UI tests if a TypeScript fallback ever becomes necessary.

## 3. Data model

### 3.1 Sign convention

Every line stores a **signed** amount: money leaving an account is negative, money arriving is positive. The arrows in requirements §4.1 map directly onto this:

- `Person: Anna → Expense: Kitchen, 50 €` becomes the lines `Anna −50,00` and `Kitchen +50,00`.

The **raw balance** of an account is the sum of its lines. The sum over all accounts is always 0.

The **displayed balance** converts the raw balance according to the account kind:

| Kind    | Displayed balance | Meaning when positive                        |
| ------- | ----------------- | -------------------------------------------- |
| asset   | raw               | money is sitting there                       |
| expense | raw               | spent                                        |
| income  | −raw              | earned                                       |
| person  | −raw              | **the event owes the person** (net advances) |
| opening | −raw              | opening counts brought in                    |

Check against Bob from requirements §4.1:

- He received 300 € of tickets: line `+300`.
- He advanced 120 € for food: line `−120`.
- Raw balance `+180`, displayed `−180,00 €`, which means "Bob owes 180,00 €". ✓

### 3.2 Schema

The schema is [`crates/lancelot-core/migrations/0001_initial.sql`](../crates/lancelot-core/migrations/0001_initial.sql). The format is **not frozen** until the first real event is entered: until then this file is edited directly, the sample is regenerated, and files from earlier development builds don't open. From that first real event on, it is never edited again, and every change is a new migration, added to `MIGRATIONS` with `SCHEMA_VERSION` raised. [`fixtures/events/sample-v1.lancelot`](../fixtures/events/sample-v1.lancelot) proves it: a file with rows in every table, its bytes pinned by a hash. [`tests/sample_file.rs`](../crates/lancelot-core/tests/sample_file.rs) opens a copy with its figures written out, keeps working on it, and fails if a new file's schema differs from the migrated sample's. Every table is `STRICT`. `PRAGMA application_id` is `0x4C4E434C` (`LNCL`), and `PRAGMA user_version` is the schema version.

Dates are ISO `YYYY-MM-DD` text, timestamps RFC 3339 UTC, enums their lower-case names, and amounts integer cents. Ids are never reused, so the change log stays unambiguous.

**Why the person data is separate:** a person is an account of kind `person`, because that is how their net balance falls out of the ledger. The bank details live in `person`, so the `account` table stays uniform.

### 3.3 Enforcing balance (R-COR-2)

SQLite can neither check an aggregate in a CHECK constraint nor defer a trigger until commit, so balance is not a database rule. It is checked in three places: when a transaction is built, before each commit, because `Store::change` reads the write back into a `Book` and commits only if it equals the one written (§4.1), and whenever a file is loaded. The column-level SQL CHECKs stay.

**The rules, checked in the `Book` and again by the integrity check.** `TxnKind` is only a label:

- A transaction has at least two non-zero lines on known accounts, summing to zero.
- The `opening` account appears only in `opening` transactions, which are that account's line and one asset line.
- `refund` comes only from a cancellation.
- No new transactions are allowed when the event is closed.

**The forms are stricter than the file** (`build_lines`): an expense pays one or more expense lines from an asset or person, and income is the reverse. A settle has one person and one asset, with its direction stored as the sign of the lines. A transfer goes from any account to any other except the opening account. `entry_of` is their reverse: a stored transaction no form can show has no `entry` and opens read-only.

### 3.4 Generated data and other files

**Ticket income** (R-TKT-6, R-TKT-7) is generated by the `Book` after every change: one `income` transaction per tier of the latest import (the one with the latest date) (`Tickets −gross`, `Ticket fees +fee` when there is one, the ticket account `+net`) and one per manual sale (`Tickets →` its destination). A refund is one more, of kind `refund`: `Ticket refunds +refund`, payer `−refund`, per cancellation (R-TKT-15). The income of a tier counts its cancelled tickets that left the sheet too. Each keeps its id while its tier, sale or cancellation exists, so attachments and links stay. `txn.origin` marks them; `save_txn` and `delete_txn` refuse them. The tickets are the source of truth and the rows only keep the file readable without Lancelot (R-NF-4): opening a file regenerates them and writes any difference with its change log entries, after the backup. Changed money is reported as a warning, changed wording is not. `read_book` keeps the result in memory only.

**Reconciliation** (M10) has one row per account and date. Its amount on a net-movement account is the net movement. Closing counts and statements are compared through one function, `book_figure` in `ledger.rs`. Generated ticket income stands for money that came in over time but carries the date of the latest import, so on the ticket account that function leaves those lines out and takes the income of the sheets known by the date instead. On a net-movement account it leaves out opening counts. A count may be below zero only on a bank account.

**Other event files** (R-EVT-3, R-ISO-4) are read with `store::read_book`: read-only, without the lock, and refused as `already_open` while another window holds them. It cannot migrate, so a second schema version must make it migrate a copy; a compile-time assert is the reminder. `Book::from_template` keeps the source's ids, which only mean something within a file; `Book::copy_from` gives what it adds new ones.

## 4. Core crate

### 4.1 Commands and queries

`Book` holds the whole event in memory and enforces every domain rule. Each change is applied to a copy, the `Ledger` is recomputed, and the copy replaces the original only if both succeed. A closed event rejects every change except reopening.

`Store` holds the open file and a `Book`. A command is a closure over the `Book`, passed to `Store::change`, which:

1. applies the command to a copy of the `Book`;
2. writes the difference between the two books in one SQL transaction, with one change log entry per changed row;
3. reads the file back inside that transaction and compares it with the copy, including the generated entries as stored, since loading regenerates them;
4. commits, which also enforces the foreign keys, and only then keeps the copy.

A command therefore lives in `Book`, whatever its size, and a new one needs a Tauri command, an API wrapper and a line in the dev bridge. `Store` has its own methods only where a command needs the clock or bytes from outside the book: closing, importing tickets and attaching files. `Service::change` wraps `Store::change` and returns the new state; `Service::add` also returns a new account's id.

Writing the difference, rather than having each command write its own rows, keeps the change log complete for every command. Foreign keys are deferred during the write, so each table is handled in one place and in any order. Only checkpoints keep an order of their own, because a checkpoint can take the date another one just left. Loading goes the other way: the rows become `BookParts`, and `Book::from_parts` checks everything a hand-edited file could get wrong.

The settle direction is explicit, and the UI pre-fills it from the person's balance. Deriving it from the balance would let an edit to an older transaction silently turn a reimbursement into a repayment.

`Ledger` is a pure value computed from the accounts and transactions. It holds:

- `raw_balance(account)` and the displayed `balance(account)`.
- `person(p)` with these figures:
  - `advanced`: money that left the person in non-settle transactions.
  - `held`: money that reached the person in non-settle transactions.
  - `reimbursed` and `repaid`: the person's settle transactions in each direction.
  - `net = advanced − held − reimbursed + repaid`. A positive value means the event owes the person.
- `where_the_money_is()` always sums to `result()`, which is a property test.

### 4.2 Errors

`DomainError` is an enum with `thiserror`. Each variant has a stable `code` (for example `unknown_account`, `wrong_account_kind`, `non_positive_amount`, `same_account`, `event_closed`, `closing_blocked`, `duplicate_name`) and an optional `field` path (for example `lines[2].amount`), so the UI can mark the right input.

Settling more than the open balance is not an error, because partial and late corrections happen. It does produce a `SettleExceedsBalance` warning in the result of `save_txn`.

### 4.3 Tests (R-COR-6)

Unit tests for every module, property tests with `proptest` for the money invariants and for "after any sequence of changes the file holds exactly the book", file tests for bad files, locking, triggers and tampering, and the example event from `UI.fig` (`tests/common/mock.rs`), which must keep reaching **+2.027,50 €** after saving and reopening.

## 5. IPC contract

- **Commands** are Tauri `#[command]`s in `app/src-tauri/src/commands.rs`. They only lock the app state and call `Service` in `crates/lancelot-service`, which does the work and has no Tauri dependency, so it is tested without it. Each returns `Result<T, ApiError>`, where `ApiError = { code, message, field?, report? }`: `field` names the input for domain errors, and `report` carries the integrity report of a damaged file.
- **Every event command returns a fresh `OpenEvent`**: `{ revision, path, fileName, snapshot, integrity, backupError }`. The `snapshot` (`EventSnapshot` in the core) holds every figure the UI shows. At ≈100 transactions it is a few tens of kilobytes. In return, the UI never has to patch state or derive anything about money. `revision` grows with every change, so a late response cannot overwrite a newer one.
- **A `TxnView` says what a transaction means**, so the UI does not read signs and account kinds: `entry` (what the form shows, with `build_lines(entry) == lines` for every typed-in entry), `who`, `flow` (what the lines do to the result) and `needsReceipt`. Generated entries show in the same form, read-only.
- **Types** for everything that crosses the boundary are generated from Rust with `ts-rs` into `app/src/lib/api/generated/`, one file per type, whenever `cargo test` runs. The files are checked in; CI fails if they differ from what the Rust types produce. `Cents`, ids and counts are mapped to `number`, dates and timestamps to `string`.
- **One window holds one open event.** Opening or creating another closes the current one first, which releases its lock.

`backups(event)` lists the backups of an event file: the open one, or one that cannot be opened (the store reads its UUID without opening it). `restoreBackup(event, backup, target)` only accepts a backup of that event. It copies it to `target`, never over an existing file, and opens the copy.

**Menu and keyboard.** New Event (⇧⌘N), Open (⌘O), Close (⌘W) and Settings (⌘,) are native menu items (`app/src-tauri/src/lib.rs`); they send a `menu` event that `app/src/lib/shortcuts.ts` turns into the action. ⌘1–9 go through the sidebar's pages and cost centres, and Esc closes the right-hand panel.

**Saving edits.** Fields save when they are left (blur or Enter). Event commands go through `session.change`, which runs them one after another and builds each from the latest state, so two quick edits can never overwrite each other.

**Shared styles.** `lib/styles/forms.css` holds what the panels' forms share, as `form-*` classes. Layout stays in each component.

**Dev bridge.** In a plain browser (`pnpm dev`), `lib/api/dev.ts` sends the commands to `crates/lancelot-bridge`, through Vite's proxy at `/__bridge`, instead of Tauri. The bridge runs the real `Service` on real event files in a temporary folder, on `127.0.0.1:5174`, so screens are checked against the real core and nothing about money is imitated. It is never part of the app, and adding a command to the app means an entry in its `commands.rs`, which a test enforces. Start it with `cargo run -p lancelot-bridge`, or the `bridge` entry in `.claude/launch.json`. To run a second pair beside it, set `LANCELOT_BRIDGE_PORT` for both the bridge and Vite, as the `bridge-alt` and `ui-alt` entries do.

- **Scenarios:** `POST /__bridge/reset/<name>`, with the header `X-Lancelot-Bridge`, starts over from `start` (the default, nothing open), `example`, `empty`, `conflict` or `closed`. The recent events always include a damaged file and one that is already open. Reload the page afterwards. Add a scenario in `scenarios.rs`.
- **Stand-ins:** the native dialogs never ask. Open gives the example event, a save gives a new path in the folder, receipts come from `fixtures/receipts` and sheets from `fixtures/tickets` in turn. Files dropped onto the page are uploaded to the folder. Opening an attachment in its app and revealing a file in Finder do nothing.
- **Guard:** a POST without that header is refused, so other pages open in the browser cannot call commands that write files.

**Attachments** are served through a custom URI scheme, `lancelot://localhost/attachment/<id>` (built with `convertFileSrc`), registered in `app/src-tauri/src/lib.rs`. `<img>` and `<iframe>` load them directly, without base64 over IPC; WebKit shows images, PDFs and HEIC itself. _Open_ writes the file to a temporary folder under its own name and opens it with the system's default app.

**Drag and drop** uses Tauri's webview drag-drop events, which give file paths; the shell reads the files when the transaction is saved, so a dropped file is only stored once the entry is saved.

## 6. Files

### 6.1 Opening and writing

- **Journal mode** is `DELETE`, not WAL. WAL leaves `-wal` and `-shm` files next to the user's file, which confuses users and cloud sync. The rollback journal exists only while a write is in progress.
- **Durability:** `synchronous = FULL` and `foreign_keys = ON`.
- **Single writer:** `locking_mode = EXCLUSIVE`, taken with `BEGIN IMMEDIATE; COMMIT` before anything else touches the file. A second instance, or a second window on the same file, gets `SQLITE_BUSY`, which becomes "already open". The lock is released when the store is dropped.
- **Creating:** the new file is written next to its target under a temporary name and hard-linked into place, which fails if the target exists. Where hard links are not supported, it is renamed after checking again. A crash never leaves a half-written event file, and an existing file is never overwritten.
- **Autosave:** every command commits immediately. There is no Save button and no unsaved state, and every committed command is durable.
- **Checking the file on open:**
  - `application_id` must be `LNCL`, or the file is rejected as not an event file.
  - A `user_version` higher than the app supports means the file is refused, with a message that a newer Lancelot is needed.
  - Next, the file is backed up and migrated if needed, and the full integrity check runs (R-COR-5).
  - A file that does not load into a `Book` is refused with the integrity report: as `damaged` when the file itself is harmed (SQLite pages, foreign keys, balance, attachment hashes), and as `inconsistent` when it is intact but its contents fail the rules. A file that loads but has issues opens, and the report is shown.

### 6.2 Backups (R-NF-5)

- **Where:** `<backup dir>/<event_uuid>/<UTC timestamp>.lancelot`, written with `VACUUM INTO`, which gives a consistent copy even while the file is open. The core takes the backup folder as an option; the app passes `<app data>/backups`. The UUID is validated when it is read, so a damaged file cannot point backups elsewhere.
- **When:**
  - on open, once the file has passed the integrity check (a damaged file is never backed up, so retrying it cannot push the good backups out);
  - before every migration;
  - after a write, if the last backup is older than 10 minutes.
- **How many:** the last **N = 20** are kept. N is an app setting.
- **Failures:** a failed backup after a change never fails the change, which is already saved. The store keeps the error so the app can show it.
- **Where else:** backups can be listed and restored from Settings. A restore opens the backup as a copy saved under a new name, and it never overwrites silently.

### 6.3 Migrations

Migrations are forward-only, numbered SQL files in `crates/lancelot-core/migrations/NNNN_name.sql`, embedded at compile time. On open, the store takes a backup and then runs each pending migration in its own transaction, which also sets `user_version`.

### 6.4 App settings

The settings are stored as JSON in `<app data>/settings.json`, written atomically. A missing or damaged file gives the defaults, so it can never keep the app from starting:

| Setting         | Default                                                                     |
| --------------- | --------------------------------------------------------------------------- |
| `numberFormat`  | from the OS locale: `de` for languages with a decimal comma, `en` otherwise |
| `backupCount`   | 20 (1–100)                                                                  |
| `reducedMotion` | follows the system until it is set                                          |
| `appearance`    | `system`, otherwise `light` or `dark`                                       |
| `recent`        | path, name and last opened time of the last 10 events                       |

Event settings (name, dates, default reimbursement account) live inside the event file. Data sources have their own file beside the settings (§10).

## 7. UI

- **Stack:** Svelte 5 (runes), SvelteKit 3 with `adapter-static` in SPA mode (`ssr = false`, fallback `index.html`), TypeScript and Vite. The package manager is **pnpm**, pinned through `packageManager` in `app/package.json`. The Tauri CLI comes from `@tauri-apps/cli` as a dev dependency, so nothing else needs a global install.
- **State:** `session` in `lib/state/session.svelte.ts` holds the `OpenEvent` and the settings as `$state`, replaced wholesale after every command. Each screen derives its view from it, for example sorting and filtering, but never sums of money. UI-only state, such as the open panel, filters and the draft form, stays local to each screen.
- **Styling:**
  - `docs/design/tokens.css` is copied to `app/src/lib/styles/tokens.css` by `pnpm tokens`, so the design file stays the source. CI fails if the copy is out of date (`pnpm tokens:check`).
  - Component styles live in the components, as plain CSS using the tokens.
  - There is no CSS framework.
- **Font:** Familjen Grotesk (OFL), in weights 400 and 600, from `@fontsource/familjen-grotesk` (latin and latin-ext). Vite bundles the `woff2` files into the build, so the app works fully offline.
- **Display formatting:** `format.ts` mirrors the core's `format_amount` and `format_date`. `formatAmount` is tested against `fixtures/amount-format.json`, the same cases the Rust formatter is tested against. It is the one piece of money code in TypeScript, and it only turns cents into strings, never strings into cents.
- **Dates** are text fields that follow the number-format setting (`31.12.2026` or `2026-12-31`). Input accepts both and is parsed in Rust (`fixtures/date-input.json`). The native date picker is not used, because it follows the OS locale rather than the setting.
- **Routes** are the folders in `app/src/routes`. `/design` shows the components in their states, in development builds only.

## 8. Decisions

| Topic           | Decision                                                                                                                                                        | Reason                                                                                                                                                                                                     |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PDF export      | **The print dialog for now** (M4): a print layout of the page, saved as PDF from macOS's dialog. Typst stays the plan for the polished settlement report (M13). | No new dependency and little code; the print layout uses the app's own type and tokens. The cost is one click in the dialog. Typst gives deterministic files without the dialog, at ≈10–15 MB more binary. |
| Charts          | **No library. Hand-written SVG components** for bars and stacked bars                                                                                           | The design needs only simple bars, and the data is tiny. The sales over time (M8) are stacked bars too.                                                                                                    |
| HEIC preview    | **Native WebKit rendering** on macOS (WKWebView shows HEIC and PDF directly). On other platforms, a fallback to "open externally".                              | No conversion code, and the primary platform is macOS (R-NF-1).                                                                                                                                            |
| Amount parser   | **Rust only**, a synchronous IPC command, with shared fixtures                                                                                                  | One implementation in the money path (R-COR-1).                                                                                                                                                            |
| TS types        | **`ts-rs`**, with the generated file checked in                                                                                                                 | Stable and simple. Commands stay hand-written wrappers.                                                                                                                                                    |
| Dates           | **`time` crate**. ISO dates as TEXT, timestamps as RFC 3339 UTC                                                                                                 | Readable without Lancelot (R-NF-4).                                                                                                                                                                        |
| SQLite          | **`rusqlite`** with the `bundled` feature                                                                                                                       | No system SQLite dependency, and it is synchronous, which suits the small data size.                                                                                                                       |
| HTTP client     | **`ureq`**, in `lancelot-service`                                                                                                                               | Small and synchronous, like the rest of the service, and only data source pulls use it.                                                                                                                    |
| Tokens          | **`keyring`**, in the Tauri shell                                                                                                                               | The system's keychain holds them, so they are never in a file. The service only sees a `TokenStore`, so its tests need no keychain.                                                                        |
| Package manager | **pnpm**                                                                                                                                                        | Strict dependency resolution and a fast install. pnpm 12 also skips packages released in the last week, which guards against compromised releases.                                                         |

## 9. Open for later milestones

- **Code signing** (after v0.1.0): it needs an Apple Developer account.

## 10. Plugins

A plugin adds records, screens and proposals for one kind of group's workflow. How to write one is in [Writing a plugin](plugins.md). Today plugins are **built in**: Svelte code in `app/src/lib/plugins/<id>/`, listed in `registry.ts` and shipped with the app. Nothing loads third-party code yet. The boundary is the one a sandboxed loader would need later, so adding that loader should not change a plugin much.

**Rules**

1. **The file works without plugins.** An event opens and settles correctly with any plugin missing, and its records are kept untouched. So a plugin's data never takes part in a balance, a result or a closing check.
2. **No code in event files.** A file holds a plugin's data, never its code, so opening a shared event can't run anything.
3. **Money moves only through what the treasurer confirms.** A plugin can propose an entry, which opens the normal form and saves through `save_txn`, or a `TicketBatch`, which opens the import's review and saves through `tickets_import`. Either way the core does the rest, and generated income stays the core's.
4. **Plugins only use `PluginContext`.** A plugin imports `#lib/plugins/api.ts`, shared components and formatting, never the session or the command API. A test enforces this.
5. **The core doesn't interpret records.** It checks the shape and enforces closing, and stores the value as JSON. A plugin validates its own values and tests its own figures, the exception to "every figure comes from the core", allowed because these figures never affect the ledger.

**Storage.** `plugin_record` has one row per plugin and key. `Book::apply_plugin_changes` applies one plugin's changes together, so they go through `Store::change` like any command: change log, read-back and backups included. The `scope` says what a template does: `setup` is copied, `records` is not. Templates keep ids, so a `setup` value may refer to accounts and cost centres. `copy_from` doesn't copy plugin data.

**Slots.** `Plugin` in `api.ts` can provide `CentreSection`, shown on a cost centre's page when `attachedTo(ctx, centre)` holds; `TicketSection`, shown on the Tickets page while the event is open; `SettingsSection`, shown under its entry in Settings → Plugins; and `ReportSection`, printed in the settlement report before the entries, unless `inReport(ctx)` says it has nothing. The report's toolbar has a switch for each one, like the one for receipts, so it can be left out of a printout. A report section only shows.

**Proposals.** `proposeEntry` takes an expense, an income or a transfer, with its date, description and notes when known, a payer or destination when it isn't the default account, and an amount per line for a split. It opens the normal form on the Ledger, so the form and the core's checks are the same as for anything typed in. A settle isn't offered: that needs a person's balance, which plugins don't see.

**Reading files.** `readTable(path, columns)` reads an `.ods`, `.xlsx` or `.csv` in native code and returns only those columns, with the file's name and hash and the numbers of rows holding a spreadsheet error. The header is the first row holding all the columns, so lines above it, as in a bank's export, don't matter. Number cells come in the app's number format and date cells as ISO, so `parseAmount` and `parseDate` read them, and both also read what a CSV holds when it is written the same way. A CSV written in another number format is refused or misread as a thousands separator would be, so a plugin reading one should show what it read before anything is saved. Paths come from `chooseFile` or a drop; a sandboxed loader has to replace them with handles, since a path lets a plugin name any file.

**Ticket sources.** The core owns tiers, imports, conflicts, cancellations, manual sales and the income they generate, and doesn't know where tickets come from. It takes a `TicketBatch`: a source name, a hash and one tier and order per ticket, so personal data can't reach it (R-TKT-17). Reading is the plugin's. `ticket-sheet` chooses the file, keeps its column names as a `setup` record and reads those two columns through `readTable`, listing the rows it can't use. `ticket-api` builds its batch from a data source's rows instead. With both plugins off, imports already made keep counting and manual sales still work.

**Showing what is a plugin.** The app draws `PluginFrame`, a card, around a plugin's content, with its name and a "Plugin" tag, so a plugin can't leave it out. Its own panels sit outside the frame.

**Switching off.** Settings has a switch per plugin and per event, on by default. Off hides the plugin's slots and keeps its records. It is stored as the app's own record, `host:off`, in the plugin's namespace, with scope `setup` so a template copies the choice. Plugins don't see it.

**Data sources.** A plugin can pull figures from a provider without any provider's details being in the repository. The provider shapes its data, so a source is only names: a URL, a header for the token, a top-level list and which field of an item holds what. A plugin declares the fields it wants as `dataSource`; the app draws the form under it in Settings and hides its `TicketSection` until it is set up. The details are in `data-sources.json` in the app's data folder, by plugin id, and never in an event file, since those get shared. The token is in the system's keychain, as the header's whole value, and is never handed back to the UI. `lancelot_service::sources` makes the request in Rust, so a plugin never sees the token: only `https`, or `http` to this computer, no redirects, and a 30-second limit. The request runs outside the lock on the app state, so a slow provider doesn't hold up the app. Only the fields asked for leave it, so anything else in a response is never kept (R-TKT-17). What a pull imports records the source's label and a hash of the rows, not the URL. Without a network or a source, everything else works as ever (R-NF-3). A provider that can't serve this shape needs a small script on its side, not code here.

**Not yet:** a loader and sandbox, which waits for a plugin that needs it.
