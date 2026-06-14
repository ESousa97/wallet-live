-- A real wallet holds cash. `balance` is the user's available fiat funds, debited
-- on buys and credited on deposits/sells. NUMERIC for exact money; never negative
-- (enforced by the application inside a transaction, and by the CHECK below).
ALTER TABLE users
    ADD COLUMN balance NUMERIC NOT NULL DEFAULT 0;

ALTER TABLE users
    ADD CONSTRAINT users_balance_non_negative CHECK (balance >= 0);
