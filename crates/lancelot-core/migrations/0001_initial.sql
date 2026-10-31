-- Lancelot event file, schema version 1. See docs/technical-design.md §3.
-- Frozen: a change is a new migration, never an edit here (tests/sample_file.rs).

CREATE TABLE meta (                    -- event_uuid, created_at, created_by, next_*_id
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

CREATE TABLE cost_centre (             -- R-CC-1, R-CC-2
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL
) STRICT;

CREATE TABLE account (
  id    INTEGER PRIMARY KEY,
  kind  TEXT NOT NULL CHECK (kind IN ('asset','person','income','expense','opening')),
  name  TEXT NOT NULL,
  role  TEXT UNIQUE CHECK (role IN ('tickets','ticket_fees','ticket_refunds')),
  asset_type TEXT CHECK (asset_type IN ('bank','cash')),   -- set exactly for assets
  net_movement INTEGER NOT NULL DEFAULT 0 CHECK (net_movement IN (0,1)),  -- R-ISO-2: only an asset
  cost_centre_id INTEGER REFERENCES cost_centre(id),       -- what a new line of this category starts with
  CHECK ((kind = 'asset') = (asset_type IS NOT NULL)),
  CHECK (kind = 'asset' OR net_movement = 0)
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
  closing_overrides TEXT NOT NULL DEFAULT '[]',  -- JSON
  ticket_tier_column  TEXT NOT NULL DEFAULT 'Ticket',     -- R-TKT-3
  ticket_order_column TEXT NOT NULL DEFAULT 'Order ID',
  ticket_date_column  TEXT,
  tickets_enabled     INTEGER NOT NULL DEFAULT 1 CHECK (tickets_enabled IN (0,1)),
  ticket_account_id   INTEGER REFERENCES account(id),     -- where the sheet's money arrives
  drink_supplier      TEXT,                               -- R-DRK-5
  price_per_strich    INTEGER CHECK (price_per_strich >= 0),  -- R-DRK-6
  drink_cost_centre_id INTEGER REFERENCES cost_centre(id),
  pfand_category_id   INTEGER REFERENCES account(id)      -- R-DRK-2
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
  kind         TEXT NOT NULL CHECK (kind IN ('expense','income','transfer','settle','opening','refund')),
  date         TEXT NOT NULL,          -- ISO date
  description  TEXT NOT NULL DEFAULT '',
  counterparty TEXT,
  notes        TEXT,
  origin       TEXT NOT NULL DEFAULT 'manual'
               CHECK (origin IN ('manual','ticket_import','ticket_sale','ticket_refund')),
  origin_id    INTEGER,                -- the tier, manual sale or cancellation it is generated from (R-TKT-6)
  CHECK ((origin = 'manual') = (origin_id IS NULL)),
  UNIQUE (origin, origin_id)
) STRICT;

CREATE TABLE budget_line (             -- R-BUD-1; planning data only (R-BUD-5)
  id          INTEGER PRIMARY KEY,
  category_id INTEGER NOT NULL REFERENCES account(id),
  description TEXT NOT NULL,
  amount      INTEGER CHECK (amount >= 0),        -- the estimate as an amount,
  quantity    INTEGER CHECK (quantity > 0),       -- or as quantity × unit price
  unit_price  INTEGER CHECK (unit_price >= 0),
  complete    INTEGER NOT NULL DEFAULT 0 CHECK (complete IN (0,1)),
  cost_centre_id INTEGER REFERENCES cost_centre(id),
  CHECK ((amount IS NULL) = (quantity IS NOT NULL) AND (quantity IS NULL) = (unit_price IS NULL))
) STRICT;

CREATE TABLE drink_product (           -- R-DRK-5; the kept units and the cost are derived
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL,
  taken      INTEGER NOT NULL CHECK (taken >= 0),
  returned   INTEGER NOT NULL CHECK (returned >= 0 AND returned <= taken),
  unit_price INTEGER NOT NULL CHECK (unit_price >= 0)
) STRICT;

CREATE TABLE ticket_tier (             -- R-TKT-1, R-TKT-10
  id     INTEGER PRIMARY KEY,
  name   TEXT NOT NULL,
  price  INTEGER NOT NULL CHECK (price >= 0),
  fee_bp INTEGER NOT NULL CHECK (fee_bp >= 0 AND fee_bp < 10000),
  quota  INTEGER CHECK (quota > 0)
) STRICT;

CREATE TABLE ticket_tier_alias (       -- R-TKT-2
  tier_id  INTEGER NOT NULL REFERENCES ticket_tier(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  alias    TEXT NOT NULL,
  PRIMARY KEY (tier_id, position)
) STRICT;

CREATE TABLE ticket_import (           -- R-TKT-5; the file itself is not stored (R-TKT-17)
  id          INTEGER PRIMARY KEY,
  filename    TEXT NOT NULL,
  sha256      TEXT NOT NULL,           -- lower-case hex
  imported_at TEXT NOT NULL,           -- RFC 3339 UTC
  date        TEXT NOT NULL            -- the day the counts stand for (R-TKT-8)
) STRICT;

CREATE TABLE ticket_import_row (       -- tickets per order and tier; order IDs only (R-TKT-18)
  import_id INTEGER NOT NULL REFERENCES ticket_import(id) ON DELETE CASCADE,
  order_id  TEXT NOT NULL,
  tier_id   INTEGER NOT NULL REFERENCES ticket_tier(id),
  count     INTEGER NOT NULL CHECK (count > 0),
  PRIMARY KEY (import_id, order_id, tier_id)
) STRICT;

CREATE TABLE ticket_manual_sale (      -- R-TKT-7
  id        INTEGER PRIMARY KEY,
  tier_id   INTEGER NOT NULL REFERENCES ticket_tier(id),
  count     INTEGER NOT NULL CHECK (count > 0),
  destination_id INTEGER NOT NULL REFERENCES account(id),  -- an asset or a person
  date      TEXT NOT NULL,
  reference TEXT
) STRICT;

CREATE TABLE ticket_cancellation (     -- R-TKT-13, R-TKT-14; conflicts are derived from the imports
  id        INTEGER PRIMARY KEY,
  tier_id   INTEGER NOT NULL REFERENCES ticket_tier(id),
  count     INTEGER NOT NULL CHECK (count > 0),
  order_id  TEXT,                      -- a sheet order, or
  sale_id   INTEGER REFERENCES ticket_manual_sale(id),  -- a manual sale
  gone      INTEGER NOT NULL DEFAULT 0 CHECK (gone BETWEEN 0 AND count),  -- confirmed as left the sheet
  note      TEXT,
  refund_bp INTEGER CHECK (refund_bp BETWEEN 0 AND 10000),  -- NULL: a correction, without a refund
  date      TEXT,
  payer_id  INTEGER REFERENCES account(id),                 -- NULL: whoever received the money
  CHECK ((order_id IS NULL) <> (sale_id IS NULL)),
  CHECK ((refund_bp IS NULL) = (date IS NULL)),
  CHECK (refund_bp IS NOT NULL OR (payer_id IS NULL AND order_id IS NOT NULL AND gone = count))
) STRICT;

CREATE TABLE line (
  id           INTEGER PRIMARY KEY,
  txn_id       INTEGER NOT NULL REFERENCES txn(id) ON DELETE CASCADE,
  position     INTEGER NOT NULL,
  account_id   INTEGER NOT NULL REFERENCES account(id),
  amount       INTEGER NOT NULL CHECK (amount <> 0),
  budget_line_id INTEGER REFERENCES budget_line(id),   -- R-BUD-2
  cost_centre_id INTEGER REFERENCES cost_centre(id),   -- R-CC-1
  UNIQUE (txn_id, position)
) STRICT;
CREATE INDEX line_account ON line(account_id);
CREATE INDEX line_budget_line ON line(budget_line_id);
CREATE INDEX line_cost_centre ON line(cost_centre_id);

CREATE TABLE closing_count (           -- R-ISO-3; opening counts are 'opening' transactions
  account_id INTEGER PRIMARY KEY REFERENCES account(id),
  date       TEXT NOT NULL,
  amount     INTEGER NOT NULL            -- below zero only on a bank account
) STRICT;

CREATE TABLE reconciliation (          -- R-ACC-4; on a net-movement account the amount is its net movement
  id         INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES account(id),
  date       TEXT NOT NULL,
  amount     INTEGER NOT NULL,
  UNIQUE (account_id, date)
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
  entity     TEXT NOT NULL,            -- 'event','account','person','txn','attachment','closing_count','budget_line',
                                       -- 'ticket_tier','ticket_import','ticket_manual_sale','ticket_cancellation',
                                       -- 'cost_centre','drink_product','reconciliation',
                                       -- 'ticket_columns','ticket_settings','drink_settings'
  entity_id  INTEGER NOT NULL,
  op         TEXT NOT NULL CHECK (op IN ('create','update','delete','close','reopen')),
  before     TEXT,                     -- JSON
  after      TEXT                      -- JSON
) STRICT;
