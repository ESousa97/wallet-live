-- Catálogo mínimo para as baterias de integração: um ativo cotado, com preço
-- que cabe em `MONEY_SCALE` e não é redondo, para que um erro de escala ou de
-- arredondamento apareça na asserção em vez de se esconder atrás de um "10,00".
--
-- Fica em `tests/fixtures/` e não é compartilhado com o fixture de
-- `src/routes/fixtures/`: os testes de unidade e os de integração precisam
-- poder mudar de estado inicial sem quebrar um ao outro.
INSERT INTO assets (name, unit_value) VALUES
    ('bitcoin', 325611.00),
    ('real', 1.00);
