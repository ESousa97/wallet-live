-- Time series of each user's total portfolio value (cash + holdings at market
-- price). One row per user is captured after every quotes round (scheduled or
-- manual), which is exactly when prices — and therefore the portfolio value —
-- change. The equity chart on the wallet page reads from here.
CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    id BIGSERIAL PRIMARY KEY NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users (id),
    total_value NUMERIC NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- The chart always reads "latest N points of one user".
CREATE INDEX IF NOT EXISTS idx_snapshots_user_time
    ON portfolio_snapshots (user_id, captured_at DESC);
