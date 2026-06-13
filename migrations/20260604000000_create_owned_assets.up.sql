-- owned_assets é o histórico de compras: cada linha registra que um usuário
-- comprou uma certa quantidade de um ativo por um certo valor unitário, num
-- determinado instante.
CREATE TABLE IF NOT EXISTS owned_assets (
    id BIGSERIAL PRIMARY KEY NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users (id),
    asset_id BIGINT NOT NULL REFERENCES assets (id),
    quantity DOUBLE PRECISION NOT NULL,
    -- Valor unitário pago no momento da compra (não o valor atual do ativo).
    unit_value DOUBLE PRECISION NOT NULL,
    bought_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
