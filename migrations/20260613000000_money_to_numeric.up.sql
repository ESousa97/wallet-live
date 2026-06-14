-- Money must not be stored as floating point: DOUBLE PRECISION carries rounding
-- noise (e.g. 0.1 + 0.2 != 0.3) that is unacceptable for financial values.
-- Switch the asset price to NUMERIC, which maps to `rust_decimal::Decimal` on the
-- Rust side and keeps exact decimal arithmetic end to end.
ALTER TABLE assets
    ALTER COLUMN unit_value TYPE NUMERIC USING unit_value::numeric;
