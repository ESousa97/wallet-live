ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_balance_non_negative;

ALTER TABLE users
    DROP COLUMN IF EXISTS balance;
