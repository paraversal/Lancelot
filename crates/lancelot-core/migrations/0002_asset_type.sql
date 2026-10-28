-- Schema version 2: asset accounts are either bank or cash accounts. Cash accounts are counted
-- at the start and the end. Existing files: the cash box is cash, every other asset a bank.

ALTER TABLE account ADD COLUMN asset_type TEXT CHECK (asset_type IN ('bank','cash'));

UPDATE account
SET asset_type = CASE WHEN role = 'cash_box' THEN 'cash' ELSE 'bank' END
WHERE kind = 'asset';
