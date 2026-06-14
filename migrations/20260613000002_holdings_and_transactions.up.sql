-- The course modelled `owned_assets` as an append-only purchase log and derived
-- everything (quantity owned, profit/loss) from aggregating it at read time. That
-- works for buy-only, but a real wallet also sells, so we split the concern in two:
--
--   * `holdings`     -> the current position per (user, asset): how much is owned
--                       and the average cost basis. Mutated atomically on buy/sell.
--   * `transactions` -> the immutable ledger of everything that happened (deposit,
--                       buy, sell), used for the history view and auditing.
--
-- Keeping a materialized position table makes the read queries trivial (no heavy
-- aggregation) and makes the money-moving logic an explicit, transactional update.

CREATE TABLE IF NOT EXISTS transactions (
    id BIGSERIAL PRIMARY KEY NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users (id),
    -- Constrained to the three movements the wallet supports.
    kind TEXT NOT NULL CHECK (kind IN ('deposit', 'buy', 'sell')),
    -- NULL for deposits (no asset involved).
    asset_id BIGINT REFERENCES assets (id),
    -- NULL for deposits; units traded for buy/sell.
    quantity NUMERIC,
    -- NULL for deposits; market price per unit at trade time for buy/sell.
    unit_value NUMERIC,
    -- Signed cash movement: deposit > 0, buy < 0, sell > 0.
    cash_delta NUMERIC NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS holdings (
    user_id BIGINT NOT NULL REFERENCES users (id),
    asset_id BIGINT NOT NULL REFERENCES assets (id),
    -- Units currently owned. Rows are deleted when this reaches zero, so in
    -- practice it is always strictly positive.
    quantity NUMERIC NOT NULL,
    -- Weighted-average price paid for the units currently held. Used to compute
    -- unrealized profit/loss against the asset's current market price.
    avg_cost NUMERIC NOT NULL,
    PRIMARY KEY (user_id, asset_id),
    CHECK (quantity >= 0),
    CHECK (avg_cost >= 0)
);

INSERT INTO holdings (user_id, asset_id, quantity, avg_cost)
SELECT
    user_id,
    asset_id,
    SUM(quantity::numeric),
    SUM(quantity::numeric * unit_value::numeric) / SUM(quantity::numeric)
FROM owned_assets
GROUP BY user_id, asset_id
ON CONFLICT (user_id, asset_id) DO NOTHING;

INSERT INTO transactions (user_id, kind, asset_id, quantity, unit_value, cash_delta, created_at)
SELECT
    user_id,
    'buy',
    asset_id,
    quantity::numeric,
    unit_value::numeric,
    -(quantity::numeric * unit_value::numeric),
    bought_at
FROM owned_assets;

DROP TABLE IF EXISTS owned_assets;

-- The history view always filters by user and orders by recency.
CREATE INDEX IF NOT EXISTS idx_transactions_user_created
    ON transactions (user_id, created_at DESC);
