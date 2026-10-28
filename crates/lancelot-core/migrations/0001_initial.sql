-- Lancelot event file, schema version 1. See docs/technical-design.md §3.

CREATE TABLE meta (                    -- event_uuid, created_at, created_by, next_*_id
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

CREATE TABLE account (
  id    INTEGER PRIMARY KEY,
  kind  TEXT NOT NULL CHECK (kind IN ('asset','person','income','expense','opening')),
  name  TEXT NOT NULL,
  role  TEXT UNIQUE CHECK (role IN ('verein','cash_box','opening')),
  asset_type TEXT CHECK (asset_type IN ('bank','cash')),   -- set exactly for assets
  CHECK ((kind = 'asset') = (asset_type IS NOT NULL))
) STRICT;

CREATE TABLE event (                   -- exactly one row
  id          INTEGER PRIMARY KEY CHECK (id = 1),
  name        TEXT NOT NULL,
  starts_on   TEXT,                    -- ISO date
  ends_on     TEXT,
  location    TEXT,
  notes       TEXT,
  closed_at   TEXT,                    -- RFC 3339 UTC; set while the event is closed
  default_reimbursement_account_id INTEGER NOT NULL REFERENCES account(id),
  closing_overrides TEXT NOT NULL DEFAULT '[]'   -- JSON
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
  origin       TEXT NOT NULL DEFAULT 'manual',
  sealed       INTEGER NOT NULL DEFAULT 0 CHECK (sealed IN (0,1))
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

CREATE TABLE closing_count (           -- R-ISO-3; opening counts are 'opening' transactions
  account_id INTEGER PRIMARY KEY REFERENCES account(id),
  date       TEXT NOT NULL,
  amount     INTEGER NOT NULL CHECK (amount >= 0)
) STRICT;

CREATE TABLE attachment (
  id         INTEGER PRIMARY KEY,
  txn_id     INTEGER NOT NULL REFERENCES txn(id) ON DELETE CASCADE,
  filename   TEXT NOT NULL,
  mime       TEXT NOT NULL,
  size       INTEGER NOT NULL,
  sha256     TEXT NOT NULL,            -- lower-case hex
  added_at   TEXT NOT NULL,            -- RFC 3339 UTC
  data       BLOB NOT NULL
) STRICT;
CREATE INDEX attachment_txn ON attachment(txn_id);

CREATE TABLE change_log (              -- R-TX-6
  id         INTEGER PRIMARY KEY,
  at         TEXT NOT NULL,            -- RFC 3339 UTC
  entity     TEXT NOT NULL,            -- 'event','account','person','txn','attachment','closing_count'
  entity_id  INTEGER NOT NULL,
  op         TEXT NOT NULL CHECK (op IN ('create','update','delete','close','reopen')),
  before     TEXT,                     -- JSON
  after      TEXT                      -- JSON
) STRICT;

-- R-COR-2: a transaction may only be sealed if its lines sum to zero and there are at least
-- two of them, and the lines of a sealed transaction cannot change. Writes unseal, replace
-- the lines and seal again within one SQL transaction.
CREATE TRIGGER txn_seal_check BEFORE UPDATE OF sealed ON txn
WHEN NEW.sealed = 1 BEGIN
  SELECT RAISE(ABORT, 'unbalanced transaction')
  WHERE (SELECT COALESCE(SUM(amount), 0) FROM line WHERE txn_id = NEW.id) <> 0
     OR (SELECT COUNT(*) FROM line WHERE txn_id = NEW.id) < 2;
END;

CREATE TRIGGER txn_insert_unsealed BEFORE INSERT ON txn
WHEN NEW.sealed = 1 BEGIN
  SELECT RAISE(ABORT, 'a transaction is inserted unsealed');
END;

CREATE TRIGGER line_insert_sealed BEFORE INSERT ON line
WHEN (SELECT sealed FROM txn WHERE id = NEW.txn_id) = 1 BEGIN
  SELECT RAISE(ABORT, 'transaction is sealed');
END;

CREATE TRIGGER line_update_sealed BEFORE UPDATE ON line
WHEN (SELECT sealed FROM txn WHERE id = OLD.txn_id) = 1
  OR (SELECT sealed FROM txn WHERE id = NEW.txn_id) = 1 BEGIN
  SELECT RAISE(ABORT, 'transaction is sealed');
END;

CREATE TRIGGER line_delete_sealed BEFORE DELETE ON line
WHEN (SELECT sealed FROM txn WHERE id = OLD.txn_id) = 1 BEGIN
  SELECT RAISE(ABORT, 'transaction is sealed');
END;
