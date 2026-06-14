CREATE TABLE IF NOT EXISTS owned_assets (
    id BIGSERIAL PRIMARY KEY NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users (id),
    asset_id BIGINT NOT NULL REFERENCES assets (id),
    quantity DOUBLE PRECISION NOT NULL,
    unit_value DOUBLE PRECISION NOT NULL,
    bought_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO owned_assets (user_id, asset_id, quantity, unit_value, bought_at)
SELECT
    user_id,
    asset_id,
    quantity::double precision,
    unit_value::double precision,
    created_at
FROM transactions
WHERE kind = 'buy';

DROP TABLE IF EXISTS holdings;
DROP TABLE IF EXISTS transactions;
