ALTER TABLE assets
    DROP CONSTRAINT IF EXISTS assets_unit_value_non_negative;

ALTER TABLE transactions
    DROP CONSTRAINT IF EXISTS transactions_quantity_positive;

ALTER TABLE transactions
    DROP CONSTRAINT IF EXISTS transactions_unit_value_non_negative;
