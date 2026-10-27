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
- `Person: Anna → Expense: Food, 50 €` becomes the lines `Anna −50,00` and `Food +50,00`.

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

```sql
PRAGMA application_id = 0x4C4E434C;   -- 'LNCL'
PRAGMA user_version   = 1;            -- schema version
PRAGMA foreign_keys   = ON;

CREATE TABLE meta (                    -- key/value: event_uuid, created_at, created_by_app
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

CREATE TABLE event (                   -- exactly one row
  id          INTEGER PRIMARY KEY CHECK (id = 1),
  name        TEXT NOT NULL,
  starts_on   TEXT,                    -- ISO date
  ends_on     TEXT,
  location    TEXT,
  notes       TEXT,
  state       TEXT NOT NULL DEFAULT 'planning'
              CHECK (state IN ('planning','running','settling','closed')),
  default_reimbursement_account_id INTEGER REFERENCES account(id)
) STRICT;

CREATE TABLE account (
  id        INTEGER PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('asset','person','income','expense','opening')),
  name      TEXT NOT NULL,
  role      TEXT UNIQUE,               -- system role: 'verein','cash_box','opening','ticket_fees',… or NULL
  position  INTEGER NOT NULL DEFAULT 0,
  archived  INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
  UNIQUE (kind, name)
) STRICT;

CREATE TABLE person (                  -- 1:1 extension of an account of kind 'person'
  account_id INTEGER PRIMARY KEY REFERENCES account(id) ON DELETE CASCADE,
  holder     TEXT,
  iban       TEXT,                     -- normalised: no spaces, upper case
  bic        TEXT,
  contact    TEXT,
  notes      TEXT
) STRICT;

CREATE TABLE txn (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('expense','income','transfer','settle','opening')),
  date         TEXT NOT NULL,          -- ISO date
  description  TEXT NOT NULL DEFAULT '',
  counterparty TEXT,
  notes        TEXT,
  origin       TEXT NOT NULL DEFAULT 'manual',   -- 'manual' | later 'ticket_import'
  sealed       INTEGER NOT NULL DEFAULT 0 CHECK (sealed IN (0,1)),
  created_at   TEXT NOT NULL,          -- RFC 3339 UTC
  updated_at   TEXT NOT NULL
) STRICT;

CREATE TABLE line (
  id           INTEGER PRIMARY KEY,
  txn_id       INTEGER NOT NULL REFERENCES txn(id) ON DELETE CASCADE,
  position     INTEGER NOT NULL,
  account_id   INTEGER NOT NULL REFERENCES account(id),
  amount       INTEGER NOT NULL CHECK (amount <> 0),
  memo         TEXT,
  UNIQUE (txn_id, position)
) STRICT;
CREATE INDEX line_account ON line(account_id);

CREATE TABLE cash_count (              -- R-ISO-3; the opening count also creates an 'opening' txn
  id         INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES account(id),
  kind       TEXT NOT NULL CHECK (kind IN ('opening','closing')),
  date       TEXT NOT NULL,
  amount     INTEGER NOT NULL CHECK (amount >= 0),
  txn_id     INTEGER REFERENCES txn(id),
  UNIQUE (account_id, kind)
) STRICT;

CREATE TABLE attachment (
  id         INTEGER PRIMARY KEY,
  txn_id     INTEGER NOT NULL REFERENCES txn(id) ON DELETE CASCADE,
  filename   TEXT NOT NULL,
  mime       TEXT NOT NULL,
  size       INTEGER NOT NULL,
  sha256     TEXT NOT NULL,            -- hex
  added_at   TEXT NOT NULL,
  data       BLOB NOT NULL
) STRICT;

CREATE TABLE change_log (              -- R-TX-6
  id         INTEGER PRIMARY KEY,
  at         TEXT NOT NULL,
  entity     TEXT NOT NULL,            -- 'txn','account','person','event','attachment','cash_count'
  entity_id  INTEGER NOT NULL,
  op         TEXT NOT NULL CHECK (op IN ('create','update','delete','state')),
  before     TEXT,                     -- JSON
  after      TEXT                      -- JSON
) STRICT;
```

**Why the person data is separate:** a person is an account of kind `person`, because that is how their net balance falls out of the ledger. The bank details live in `person`, so the `account` table stays uniform.

**Defaults on create:**

| Kind | Accounts | Role |
|---|---|---|
| asset | `Verein account` | `verein` |
| asset | `Cash box` | `cash_box` |
| opening | `Opening balance` | `opening` |
| income | Tickets, Drink donations, Other | |
| expense | Food, Venue, Drinks, Pfand, Tech, Other | |

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
CREATE TRIGGER line_ins_sealed BEFORE INSERT ON line
WHEN (SELECT sealed FROM txn WHERE id = NEW.txn_id) = 1
BEGIN SELECT RAISE(ABORT, 'transaction is sealed'); END;
-- … the same for UPDATE and DELETE (OLD.txn_id)
```

Every write of a transaction is one SQL transaction with these steps:
1. Unseal the transaction.
2. Replace its lines.
3. Seal it.
4. Write the change log entry.
5. Commit.

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
  model.rs       AccountKind, Account, Person, Txn, Line, TxnKind, EventState, ids
  entry.rs       TxnDraft / Entry → balanced lines (expense, income, transfer, settle, opening)
  book.rs        Book: the event in memory; every change is checked on a copy, then kept
  ledger.rs      Ledger: balances, person summaries, totals, where the money is, open items
  store/         Store (rusqlite): create/open, migrations, repositories, backups
  integrity.rs   check(&Store) -> Report
  export/        csv (payout list, transactions); pdf follows in M4
  error.rs       DomainError, StoreError
```

### 4.1 Commands and queries

`Book` (M1b) holds the whole event in memory and enforces every domain rule. Each change is applied to a copy, the `Ledger` is recomputed, and the copy replaces the original only if both succeed. A closed event rejects every change except reopening. `Store` (M1c) loads a `Book` from the file and, for each command, applies it to the `Book` first and then writes the accepted result in one SQL transaction.

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
    pub fn set_state(&mut self, to: EventState, overrides: Vec<BlockerOverride>) -> Result<()>;
    pub fn ledger(&self) -> &Ledger;                         // always up to date
}
```

```rust
impl Store {
    pub fn create(path: &Path, init: EventInit) -> Result<Store>;
    pub fn open(path: &Path) -> Result<(Store, integrity::Report)>;

    // commands: each is one SQL transaction and writes the change log
    pub fn save_txn(&mut self, draft: TxnDraft, id: Option<TxnId>) -> Result<TxnId>;
    pub fn delete_txn(&mut self, id: TxnId) -> Result<()>;
    pub fn save_account(&mut self, a: AccountDraft, id: Option<AccountId>) -> Result<AccountId>;
    pub fn save_person(&mut self, p: PersonDraft, id: Option<AccountId>) -> Result<AccountId>;
    pub fn add_attachment(&mut self, txn: TxnId, file: AttachmentInput) -> Result<AttachmentId>;
    pub fn remove_attachment(&mut self, id: AttachmentId) -> Result<()>;
    pub fn set_cash_count(&mut self, c: CashCountDraft) -> Result<()>;
    pub fn update_event(&mut self, e: EventPatch) -> Result<()>;
    pub fn set_state(&mut self, to: EventState, overrides: Vec<BlockerOverride>) -> Result<()>;

    // queries
    pub fn ledger(&self) -> Result<Ledger>;          // loads everything; ~100 txns, fine
    pub fn attachment_data(&self, id: AttachmentId) -> Result<(String, Vec<u8>)>;
}

pub struct TxnDraft { date, description, counterparty, notes, entry: Entry }

pub enum Entry {
    Expense  { payer: AccountId, lines: Vec<SplitLine> },        // payer: asset or person
    Income   { destination: AccountId, lines: Vec<SplitLine> },  // destination: asset or person
    Transfer { from: AccountId, to: AccountId, amount: Cents },  // to: asset only
    Settle   { person: AccountId, account: AccountId, amount: Cents, direction: SettleDirection },
    Opening  { account: AccountId, amount: Cents },
}
pub enum SettleDirection { Reimburse /* asset → person */, Repay /* person → asset */ }
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
  - save → reopen gives an identical `Ledger`.
- **Fixture test:** a builder in `tests/fixtures.rs` recreates the event from `UI.fig`. It asserts a result of **+2.027,50 €**, Verein account 1.757,00 €, cash box 412,90 €, Anna owed 212,40 €, Malte owed 110,00 € and Bob owes 180,00 €.
- **Snapshot tests** (`insta`) for CSV exports.

## 5. IPC contract

- **Commands** are Tauri `#[command]`s in `app/src-tauri/src/commands.rs`. Each returns `Result<T, ApiError>`, where `ApiError = { code, message, field? }` and is generated from `DomainError`.
- **Every mutation returns a fresh `EventSnapshot`.** At ≈100 transactions the snapshot is a few tens of kilobytes. In return, the UI never has to patch state or derive anything about money.
- **Types** for the snapshot and drafts are generated from Rust with `ts-rs` into `app/src/lib/api/types.ts`; the generated file is checked in and CI checks that it is up to date. `Cents` is mapped to `number`.

```ts
// app/src/lib/api/index.ts — thin typed wrappers around invoke()
eventCreate(path, init): Promise<EventSnapshot>
eventOpen(path): Promise<{ snapshot: EventSnapshot; integrity: IntegrityReport }>
eventUpdate(patch): Promise<EventSnapshot>
eventSetState(to, overrides): Promise<EventSnapshot>
txnSave(draft, id?): Promise<EventSnapshot>
txnDelete(id): Promise<EventSnapshot>
accountSave(draft, id?) / personSave(draft, id?): Promise<EventSnapshot>
attachmentAdd(txnId, filePath) / attachmentRemove(id): Promise<EventSnapshot>
cashCountSet(draft): Promise<EventSnapshot>
amountParse(input, allowNegative): Promise<ParseResult>        // uses the app's number format
ibanCheck(input): Promise<IbanResult>
integrityCheck(): Promise<IntegrityReport>
exportPayoutList(format: 'csv' | 'pdf', path): Promise<void>
settingsGet() / settingsSet(patch): Promise<AppSettings>
recentEvents(): Promise<RecentEvent[]>
```

`EventSnapshot` holds:
- `revision`
- `event` (header and state)
- `accounts`, each with its displayed balance
- `persons`, each with its summary
- `txns`, with lines and attachment metadata (no bytes)
- `totals`, `result`, `whereTheMoneyIs` and `openItems`
- `cash` (counts and discrepancy)

**Attachments** are served through a custom URI scheme, `lancelot://attachment/<id>`, which is registered in Tauri. `<img>` and `<iframe>` load them directly, without base64 over IPC.

## 6. Files

### 6.1 Opening and writing

- **Journal mode** is `DELETE`, not WAL. WAL leaves `-wal` and `-shm` files next to the user's file, which confuses users and cloud sync. The rollback journal exists only while a write is in progress.
- **Durability:** `synchronous = FULL` and `foreign_keys = ON`.
- **Single writer:** `locking_mode = EXCLUSIVE`, taken on open with `BEGIN IMMEDIATE; COMMIT`. A second instance, or a second window on the same file, gets `SQLITE_BUSY` and shows "already open". The lock is released when the window closes.
- **Autosave:** every command commits immediately. There is no Save button and no unsaved state, and every committed command is durable.
- **Checking the file on open:**
  - `application_id` must be `LNCL`, or the file is rejected as not an event file.
  - A `user_version` higher than the app supports means the file is refused, with a message that a newer Lancelot is needed.
  - Next, the integrity check runs (R-COR-5).

### 6.2 Backups (R-NF-5)

- **Where:** `<app data>/backups/<event_uuid>/<UTC timestamp>.lancelot`, written with `VACUUM INTO`, which gives a consistent copy even while the file is open.
- **When:**
  - on open, before the first write;
  - before every migration;
  - after a write, if the last backup is older than 10 minutes.
- **How many:** the last **N = 20** are kept. N is an app setting.
- **Where else:** backups can be listed and restored from Settings. A restore opens the backup as a copy saved under a new name, and it never overwrites silently.

### 6.3 Migrations

Migrations are forward-only, numbered SQL files in `crates/lancelot-core/migrations/NNNN_name.sql`, embedded at compile time. On open, the app takes a backup, runs the pending migrations in one transaction and sets `user_version`.

### 6.4 App settings

The settings are stored as JSON in `<app data>/settings.json`:

| Setting | Default |
|---|---|
| `numberFormat` | follows the OS locale |
| `backupCount` | 20 |
| `recentEvents` | path, name, last opened |

Event settings (name, dates, default reimbursement account) live inside the event file.

## 7. UI

- **Stack:** Svelte 5 (runes), SvelteKit 3 with `adapter-static` in SPA mode (`ssr = false`, fallback `index.html`), TypeScript and Vite. The package manager is **pnpm**, pinned through `packageManager` in `app/package.json`. The Tauri CLI comes from `@tauri-apps/cli` as a dev dependency, so nothing else needs a global install.
- **State:** one `snapshot` `$state` per window, replaced wholesale after every command. Each screen derives its view from it, for example sorting and filtering, but never sums of money. UI-only state, such as the open panel, filters and the draft form, stays local to each screen.
- **Styling:**
  - `docs/design/tokens.css` is copied to `app/src/lib/styles/tokens.css` by `pnpm tokens`, so the design file stays the source. CI fails if the copy is out of date (`pnpm tokens:check`).
  - Component styles live in the components, as plain CSS using the tokens.
  - There is no CSS framework.
- **Font:** Familjen Grotesk (OFL), in weights 400 and 600, from `@fontsource/familjen-grotesk` (latin and latin-ext). Vite bundles the `woff2` files into the build, so the app works fully offline.
- **Display formatting:** `format.ts` mirrors the core's `format_amount` and is covered by the shared fixture. It is the one piece of money code in TypeScript, and it only turns cents into strings, never strings into cents.
- **Routes:**

  | Route | Screen |
  |---|---|
  | `/` | start: recent events, New, Open |
  | `/event/overview` | Overview |
  | `/event/ledger` | Ledger |
  | `/event/people` | People |
  | `/event/settings` | Settings |

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
