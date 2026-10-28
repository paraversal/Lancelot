# Lancelot — Technical design

Status: draft v0.1 (milestone M0). It implements [requirements v0.7](requirements.md) and follows the [roadmap](roadmap.md). This document covers the MVP (M1–M5) in detail. Phase 2 and 3 appear only where they affect today's decisions.

## 1. Components

```
┌────────────── app/ (SvelteKit SPA, Svelte 5, TypeScript) ──────────────┐
│ screens · components · formatting for display · keyboard model         │
└───────────────▲───────────────────────────────┬────────────────────────┘
                │ EventSnapshot (JSON)          │ commands (invoke)
┌───────────────┴───────────── app/src-tauri/ ──▼────────────────────────┐
│ Tauri 2 shell: command handlers, file dialogs, drag-and-drop,          │
│ attachment protocol, app settings, window ↔ open event                 │
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
- No arithmetic on money in TypeScript. The UI receives every figure from the core, already computed, as integer cents. It only *formats* them.
- One window holds one open event. App-wide settings live outside the event files (§6.4).

## 2. Money

### 2.1 Representation
- `Cents(i64)` is a newtype with `checked_add`, `checked_sub` and `checked_mul(i64)`. An overflow is a `DomainError::Overflow`, never a wrap.
- It crosses the IPC boundary as a JSON number. All plausible values are far below 2⁵³, and the core rejects any amount above `MAX_AMOUNT = 10¹²` cents (10 billion €).

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

**Quantity × unit price** (budget lines, drink products) is an exact multiplication, so no rounding is needed.

### 2.3 Amount input (R-NF-8)

```rust
pub enum NumberFormat { De /* 1.234,56 € · 31.12.2026 */, En /* €1,234.56 · 2026-12-31 */ }

pub fn parse_amount(input: &str, fmt: NumberFormat, allow_negative: bool)
    -> Result<Parsed, AmountError>;
pub struct Parsed { pub cents: Cents, pub ambiguous: bool }
pub enum AmountError { Empty, InvalidCharacter, BadGrouping, TooManyDecimals,
                       MisplacedSign, NegativeNotAllowed, TooLarge }
pub enum SignStyle { Plain /* −12,50 € */, Always /* +12,50 € */, Magnitude /* 12,50 € */ }

pub fn format_amount(c: Cents, fmt: NumberFormat, sign: SignStyle) -> String;
```

The parser works in these steps:
1. Strip spaces and `€`. Accept one leading `-`, `−` or `+`. A sign anywhere else is `MisplacedSign`.
2. Find the decimal separator: the last `.` or `,` that is followed by 1–2 digits and nothing else.
3. Treat every other separator as a thousands separator. It must be the other character, the groups must be 3 digits, and the first group 1–3 digits.
4. Four or more digits after the last separator, or three digits after a last separator that differs from the others (`1.234,567`), are `TooManyDecimals`.
5. A single separator followed by exactly 3 digits is *ambiguous* and read according to `fmt`:
   - If it is the thousands separator of `fmt` (`1.234` in De, `1,234` in En), the amount is accepted and `ambiguous = true`, so the preview is shown in `attention`.
   - If it is the decimal separator of `fmt` (`1,234` in De, `1.234` in En), it means three decimals and is rejected as `TooManyDecimals`. A German user who types `1,234` almost certainly meant `1,23` or `12,34`, and reading it as a thousand euros would be a 1000× error.

**Where the parser runs:** in Rust, behind the synchronous command `amount_parse`. A round trip takes well under a millisecond, so the live preview needs no second implementation in TypeScript. The test cases live in `fixtures/amount-input.json`, which holds input, format, the expected result and the ambiguity flag. They are read by the Rust tests, and later also by UI tests if a TypeScript fallback ever becomes necessary.

## 3. Data model

### 3.1 Sign convention

Every line stores a **signed** amount: money leaving an account is negative, money arriving is positive. The arrows in requirements §4.1 map directly onto this:
- `Person: Anna → Expense: Kitchen, 50 €` becomes the lines `Anna −50,00` and `Kitchen +50,00`.

The **raw balance** of an account is the sum of its lines. The sum over all accounts is always 0.

The **displayed balance** converts the raw balance according to the account kind:

| Kind | Displayed balance | Meaning when positive |
|---|---|---|
| asset | raw | money is sitting there |
| expense | raw | spent |
| income | −raw | earned |
| person | −raw | **the event owes the person** (net advances) |
| opening | −raw | opening counts brought in |

Check against Bob from requirements §4.1:
- He received 300 € of tickets: line `+300`.
- He advanced 120 € for food: line `−120`.
- Raw balance `+180`, displayed `−180,00 €`, which means "Bob owes 180,00 €". ✓

### 3.2 Schema (MVP, schema version 1)

The schema is [`crates/lancelot-core/migrations/0001_initial.sql`](../crates/lancelot-core/migrations/0001_initial.sql). Every table is `STRICT`. `PRAGMA application_id` is `0x4C4E434C` (`LNCL`), and `PRAGMA user_version` is the schema version.

| Table | Holds |
|---|---|
| `meta` | Key/value pairs: `event_uuid`, `created_at`, `created_by`, and `next_account_id`, `next_txn_id`, `next_attachment_id`. Ids are never reused, so the change log stays unambiguous. |
| `event` | Exactly one row: details, the `closed` flag, default reimbursement account, and the closing overrides as JSON. |
| `account` | `id`, `kind`, `name`, and an optional unique `role` (`verein`, `cash_box`, `opening`). |
| `person` | Bank and contact details, 1:1 with an account of kind `person`. |
| `txn` | Kind, date, description, counterparty, notes, `origin` (`manual`; later `ticket_import`) and the `sealed` flag. |
| `line` | `txn_id`, `position`, `account_id`, a signed `amount` that is never 0, and a memo. |
| `closing_count` | One per asset account at most: date and counted amount (R-ISO-3). Opening counts are `opening` transactions. |
| `attachment` | File name, type, size, SHA-256, `added_at`, and the bytes as a BLOB. |
| `change_log` | `at`, `entity`, `entity_id`, `op` (`create`, `update`, `delete`, `close`, `reopen`) and the before and after values as JSON (R-TX-6). |

Dates are ISO `YYYY-MM-DD` text, timestamps RFC 3339 UTC, enums their lower-case names, and amounts integer cents.

**Why the person data is separate:** a person is an account of kind `person`, because that is how their net balance falls out of the ledger. The bank details live in `person`, so the `account` table stays uniform.

**Defaults on create:**

| Kind | Accounts | Role |
|---|---|---|
| asset | `Verein account` | `verein` |
| asset | `Cash box` | `cash_box` |
| opening | `Opening balance` | `opening` |
| income | Tickets, Drink donations, General | |
| expense | Kitchen, Drinks, Pfand, Mobility, Venue, Tech, General | |

The default reimbursement account is the Verein account.

### 3.3 Enforcing balance in the database (R-COR-2)

SQLite can neither check an aggregate in a CHECK constraint nor defer a trigger until commit. The schema therefore uses a **seal**:

```sql
-- a transaction may only be sealed if its lines sum to 0 and there are at least two
CREATE TRIGGER txn_seal_check BEFORE UPDATE OF sealed ON txn
WHEN NEW.sealed = 1 BEGIN
  SELECT RAISE(ABORT, 'unbalanced transaction')
  WHERE (SELECT COALESCE(SUM(amount), 0) FROM line WHERE txn_id = NEW.id) <> 0
     OR (SELECT COUNT(*) FROM line WHERE txn_id = NEW.id) < 2;
END;

-- lines of a sealed transaction cannot change
CREATE TRIGGER line_insert_sealed BEFORE INSERT ON line
WHEN (SELECT sealed FROM txn WHERE id = NEW.txn_id) = 1
BEGIN SELECT RAISE(ABORT, 'transaction is sealed'); END;
-- … the same for UPDATE and DELETE, and transactions must be inserted unsealed
```

Every write of a transaction is part of one SQL transaction with these steps:
1. Unseal the transaction (or insert it unsealed).
2. Replace its lines.
3. Seal it.
4. Write the change log entry.
5. Run the quick integrity check, and commit only if it passes.

Deleting a transaction also unseals it first, so the cascade can remove its lines.

The domain layer has already checked the balance before this. The trigger is the second line of defence. The integrity check also reports any transaction with `sealed = 0`, because that could only exist after a crash outside a transaction, which should be impossible.

**Further rules, checked in the domain layer and again by the integrity check:**
- An `opening` account appears only in `opening` transactions.
- A `settle` transaction has exactly one person line and one asset line. Its direction (reimburse or repay) is chosen explicitly and stored as the sign of the lines.
- An `expense` transaction has one source (asset or person) and one or more expense lines.
- An `income` transaction has one or more income lines and one destination (asset or person).
- A `transfer` transaction has exactly two lines: from an asset or person account, to an asset account. Money going to a person is always a `settle`.
- No new transactions are allowed when the event state is `closed`.

### 3.4 Phase 2 tables (outline only)

These are added by migrations; the names are fixed now so the MVP code does not paint us into a corner.
- `cost_centre`, plus `line.cost_centre_id`.
- `budget_line`, plus `line.budget_line_id`.
- `ticket_tier` and `ticket_tier_alias`.
- `ticket_import` (file name, sha256, imported_at), `ticket_import_row` (order_id, tier_id, count) and `ticket_conflict`.
- `ticket_manual_sale` and `ticket_cancellation`.
- `drink_product`, `reconciliation` and `tally`.

`txn.origin = 'ticket_import'` marks generated transactions as read-only.

## 4. Core crate

```
crates/lancelot-core/src/
  lib.rs
  money.rs       Cents, allocate, percent_bp
  amount.rs      parse_amount, format_amount, NumberFormat
  iban.rs        normalise, validate (mod 97), format in groups of four
  model.rs       AccountKind, Account, Person, Txn, Line, TxnKind, Event, ids
  entry.rs       TxnDraft / Entry → balanced lines (expense, income, transfer, settle, opening)
  book.rs        Book: the event in memory; every change is checked on a copy, then kept
  ledger.rs      Ledger: balances, person summaries, totals, where the money is, open items
  store/         Store: the event file (rusqlite)
    mod.rs         create/open, locking, migrations, commands, attachments
    read.rs        file → BookParts
    write.rs       difference between two books → rows and change log entries
    backup.rs      VACUUM INTO backups, pruning, listing
    codec.rs       dates, timestamps, enums as text
  integrity.rs   check(&Connection, Depth) -> Report
  error.rs       DomainError
  export/        csv (payout list, transactions) and pdf, from M4
```

### 4.1 Commands and queries

`Book` holds the whole event in memory and enforces every domain rule. Each change is applied to a copy, the `Ledger` is recomputed, and the copy replaces the original only if both succeed. A closed event rejects every change except reopening.

`Store` holds the open file and a `Book`. For each command it:
1. applies the command to a copy of the `Book`;
2. writes the difference between the two books in one SQL transaction, with one change log entry per changed row;
3. runs the quick integrity check inside that transaction;
4. commits, and only then keeps the copy.

Writing the difference, rather than having each command write its own rows, keeps the change log complete for every command. Loading goes the other way: the rows become `BookParts`, and `Book::from_parts` checks everything a hand-edited file could get wrong.

```rust
impl Book {
    pub fn new(details: EventDetails) -> Result<Book>;      // default accounts and categories
    pub fn add_account(&mut self, kind: AccountKind, name: &str) -> Result<AccountId>;
    pub fn rename_account(&mut self, id: AccountId, name: &str) -> Result<()>;
    pub fn add_person(&mut self, name: &str, person: Person) -> Result<AccountId>;
    pub fn update_person(&mut self, id: AccountId, name: &str, person: Person) -> Result<()>;
    pub fn save_txn(&mut self, draft: &TxnDraft, id: Option<TxnId>) -> Result<Saved>;  // Saved { id, warnings }
    pub fn delete_txn(&mut self, id: TxnId) -> Result<Txn>;
    pub fn update_event(&mut self, details: EventDetails) -> Result<()>;
    pub fn set_default_reimbursement_account(&mut self, id: AccountId) -> Result<()>;
    pub fn close(&mut self, overrides: Vec<BlockerOverride>) -> Result<()>;  // R-SET-7
    pub fn reopen(&mut self);
    pub fn ledger(&self) -> &Ledger;                         // always up to date
}
```

```rust
impl Store {
    pub fn create(path: &Path, details: EventDetails, options: StoreOptions) -> Result<(Store, Report)>;
    pub fn open(path: &Path, options: StoreOptions) -> Result<(Store, Report)>;

    // commands: the Book's, plus attachments and closing counts; each one SQL transaction
    pub fn save_txn(&mut self, draft: &TxnDraft, id: Option<TxnId>) -> Result<Saved>;
    pub fn delete_txn(&mut self, id: TxnId) -> Result<()>;
    pub fn add_attachment(&mut self, txn: TxnId, filename: &str, data: &[u8]) -> Result<AttachmentId>;
    pub fn remove_attachment(&mut self, id: AttachmentId) -> Result<()>;
    pub fn set_closing_count(&mut self, account: AccountId, date: Date, amount: Cents) -> Result<()>;
    pub fn clear_closing_count(&mut self, account: AccountId) -> Result<()>;
    // … add_account, rename_account, add_person, update_person, update_event,
    //   set_default_reimbursement_account, close, reopen

    // queries
    pub fn book(&self) -> &Book;                      // with its always-current Ledger
    pub fn attachment_data(&self, id: AttachmentId) -> Result<Vec<u8>>;
    pub fn change_log(&self, limit: usize) -> Result<Vec<ChangeLogEntry>>;
    pub fn check_integrity(&self) -> Result<Report>;  // full check, on demand
    pub fn backup_now(&mut self) -> Result<PathBuf>;
}
```

The settle direction is explicit, and the UI pre-fills it from the person's balance. Deriving it from the balance would let an edit to an older transaction silently turn a reimbursement into a repayment.

`Ledger` is a pure value computed from the accounts and transactions. It holds:
- `raw_balance(account)` and the displayed `balance(account)`.
- `person(p)` with these figures:
  - `advanced`: money that left the person in non-settle transactions.
  - `held`: money that reached the person in non-settle transactions.
  - `reimbursed` and `repaid`: the person's settle transactions in each direction.
  - `net = advanced − held − reimbursed + repaid`. A positive value means the event owes the person.
- `income_total()`, `expense_total()` and `result()` (income − expense). Category totals are the displayed balances of the income and expense accounts.
- `where_the_money_is()`, which splits the result into:
  - asset accounts,
  - money held by people (negative person balances),
  - money owed to people (positive person balances),
  - minus the opening balance.

  These always sum to `result()`. This is a property test.
- `open_items()`: non-zero person balances. M1c adds expense transactions without an attachment and the cash box closing count (missing, or with a discrepancy). Phase 2 adds reconciliation mismatches and ticket conflicts.

### 4.2 Errors

`DomainError` is an enum with `thiserror`. Each variant has a stable `code` (for example `unknown_account`, `wrong_account_kind`, `non_positive_amount`, `same_account`, `event_closed`, `closing_blocked`, `duplicate_name`) and an optional `field` path (for example `lines[2].amount`), so the UI can mark the right input.

Settling more than the open balance is not an error, because partial and late corrections happen. It does produce a `SettleExceedsBalance` warning in the result of `save_txn`.

### 4.3 Tests (R-COR-6)

- **Unit tests** for every module.
- **Property tests** with `proptest`:
  - all raw balances sum to 0 after any sequence of valid drafts;
  - `allocate` parts sum to the total and differ by at most one cent;
  - `parse_amount(format_amount(x)) == x` in both formats;
  - `where_the_money_is()` sums to `result()`;
  - after any sequence of changes the file holds exactly the book, and reopening gives it back.
- **Fixture test:** `tests/common/mock.rs` builds the event from `UI.fig`, in a `Book` or a `Store`. It asserts a result of **+2.027,50 €**, Verein account 1.757,00 €, cash box 412,90 € (counted 410,00 €), Anna owed 212,40 €, Malte owed 110,00 € and Bob owes 180,00 €, also after saving and reopening.
- **File tests:** bad files, newer schema, locking, the triggers, tampered lines and attachments, the change log and backups.
- **Snapshot tests** (`insta`) for CSV exports.

## 5. IPC contract

- **Commands** are Tauri `#[command]`s in `app/src-tauri/src/commands.rs`. They only lock the app state and call `service.rs`, which does the work and is tested without Tauri. Each returns `Result<T, ApiError>`, where `ApiError = { code, message, field?, report? }`: `field` names the input for domain errors, and `report` carries the integrity report of a damaged file.
- **Every event command returns a fresh `OpenEvent`**: `{ revision, path, fileName, snapshot, integrity, backupError }`. The `snapshot` (`EventSnapshot` in the core) holds every figure the UI shows. At ≈100 transactions it is a few tens of kilobytes. In return, the UI never has to patch state or derive anything about money. `revision` grows with every change, so a late response cannot overwrite a newer one.
- **Types** for everything that crosses the boundary are generated from Rust with `ts-rs` into `app/src/lib/api/generated/`, one file per type, whenever `cargo test` runs. The files are checked in; CI fails if they differ from what the Rust types produce. `Cents`, ids and counts are mapped to `number`, dates and timestamps to `string`.
- **One window holds one open event.** Opening or creating another closes the current one first, which releases its lock.

```ts
// app/src/lib/api/index.ts — thin typed wrappers around invoke()
appInfo(): AppInfo
settings() / setSettings(settings): AppSettings          // number format, backup count
recentEvents() / forgetRecent(path): RecentEvent[]        // with `exists` for moved files
createEvent(path, details) / openEvent(path): OpenEvent
closeFile() / currentEvent(): OpenEvent | null
updateEvent(details) / setDefaultReimbursementAccount(account): OpenEvent
checkIntegrity(): OpenEvent
backups(): BackupInfo[] / restoreBackup(backup, target): OpenEvent
parseAmount(input, allowNegative): Parsed                 // uses the app's number format
parseDate(input): { date }                                // ISO
saveTxn(draft, id?): SavedTxn / deleteTxn(id): OpenEvent         // draft.total checks the split
addAccount(kind, name) / addPerson(name): Added / renameAccount(id, name)
// saveTxn also takes `files` (paths) and `remove` (attachment ids), saved in one change
openAttachment(id) / exportAttachment(id, target)
updatePerson(id, name, person): OpenEvent / checkIban(input): { iban, formatted }
// M4b: payout and repayment lists; M5: closing counts
```

`backups(event)` lists the backups of an event file: the open one, or one that cannot be opened (the store reads its UUID without opening it). `restoreBackup(event, backup, target)` only accepts a backup of that event. It copies it to `target`, never over an existing file, and opens the copy.

**Menu and keyboard.** New Event (⇧⌘N), Open (⌘O), Close (⌘W) and Settings (⌘,) are native menu items (`app/src-tauri/src/lib.rs`); they send a `menu` event that `app/src/lib/shortcuts.ts` turns into the action. ⌘1–3 switch pages and Esc closes the right-hand panel.

**Saving edits.** Fields save when they are left (blur or Enter). Event commands go through `session.change`, which runs them one after another and builds each from the latest state, so two quick edits can never overwrite each other.

**Development mock.** In a plain browser (`pnpm dev`), `lib/api` uses `lib/api/mock` instead of Tauri: the example event and an empty one, exported as JSON by `crates/lancelot-core/tests/mock_event.rs`, plus rough stand-ins for the commands and dialogs. It is loaded only in development and never in the app.

**Attachments** are served through a custom URI scheme, `lancelot://localhost/attachment/<id>` (built with `convertFileSrc`), registered in `app/src-tauri/src/lib.rs`. `<img>` and `<iframe>` load them directly, without base64 over IPC; WebKit shows images, PDFs and HEIC itself. *Open* writes the file to a temporary folder under its own name and opens it with the system's default app.

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
  - A file that does not load into a `Book` is reported as damaged, with the integrity report. A file that loads but has issues opens, and the report is shown.

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

| Setting | Default |
|---|---|
| `numberFormat` | from the OS locale: `de` for languages with a decimal comma, `en` otherwise |
| `backupCount` | 20 (1–100) |
| `recent` | path, name and last opened time of the last 10 events |

Event settings (name, dates, default reimbursement account) live inside the event file.

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
- **Routes:**

  | Route | Screen |
  |---|---|
  | `/` | start: recent events, New, Open |
  | `/event/overview` | Overview |
  | `/event/ledger` | Ledger |
  | `/event/people` | People |
  | `/event/settings` | Settings |
  | `/design` | the components in their states, next to the System frame (development builds only) |

  Phase 2 adds `/event/budget`, `/event/tickets` and `/event/drinks`.

## 8. Decisions made in M0

| Topic | Decision | Reason |
|---|---|---|
| PDF export | **Typst as a library** (`typst` + `typst-pdf`, with a minimal `World` and the bundled font), from M4 | Deterministic, offline, typographically good, and it can follow the design system. Tauri has no portable silent print-to-PDF. The cost is ≈10–15 MB more binary, which is acceptable. |
| Charts | **No library. Hand-written SVG components** for bars and stacked bars | The design needs only simple bars, and the data is tiny. Revisit in M8 if the time-series chart gets complex. |
| HEIC preview | **Native WebKit rendering** on macOS (WKWebView shows HEIC and PDF directly). On other platforms, a fallback to "open externally". | No conversion code, and the primary platform is macOS (R-NF-1). |
| Amount parser | **Rust only**, a synchronous IPC command, with shared fixtures | One implementation in the money path (R-COR-1). |
| TS types | **`ts-rs`**, with the generated file checked in | Stable and simple. Commands stay hand-written wrappers. |
| Dates | **`time` crate**. ISO dates as TEXT, timestamps as RFC 3339 UTC | Readable without Lancelot (R-NF-4). |
| SQLite | **`rusqlite`** with the `bundled` feature | No system SQLite dependency, and it is synchronous, which suits the small data size. |
| Package manager | **pnpm** | Strict dependency resolution and a fast install. pnpm 12 also skips packages released in the last week, which guards against compromised releases. |

## 9. Open for later milestones

- **The data behind the ticket statistics** (M8): a `sales_over_time` view that is fed by import snapshots now and by sale dates later (R-TKT-8).
- **Code signing** (M5): it needs an Apple Developer account.
