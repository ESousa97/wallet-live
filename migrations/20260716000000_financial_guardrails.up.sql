-- Financial guard-rails at the schema level. The application already validates
-- these at the HTTP border, but the database is the last line of defense: no
-- write path (admin API, quote sync, manual SQL) can persist an invalid value.

-- An asset price can never be negative. A negative price would invert the math
-- everywhere it is used: a "buy" would credit the balance instead of debiting it.
ALTER TABLE assets
    ADD CONSTRAINT assets_unit_value_non_negative CHECK (unit_value >= 0);

-- Trades must record a positive quantity and a non-negative price. Both columns
-- are NULL for deposits, so the checks only bite on buy/sell rows.
ALTER TABLE transactions
    ADD CONSTRAINT transactions_quantity_positive
        CHECK (quantity IS NULL OR quantity > 0);

ALTER TABLE transactions
    ADD CONSTRAINT transactions_unit_value_non_negative
        CHECK (unit_value IS NULL OR unit_value >= 0);
