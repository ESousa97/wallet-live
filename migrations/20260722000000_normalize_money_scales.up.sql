-- Saneamento de escala monetária.
--
-- A sincronização de cotações gravava preço = 1/taxa SEM arredondar: a divisão
-- de rust_decimal preenche a mantissa inteira e o NUMERIC ficava com até 28
-- casas decimais. Produtos e somas sobre esses valores (resumo da carteira,
-- posições, snapshots do patrimônio) passam de 28 dígitos SIGNIFICATIVOS — o
-- limite do rust_decimal::Decimal — e a LEITURA falha com "value not
-- representable" (500 em /assets para quem tinha posições).
--
-- Normaliza o estado vivo para a escala canônica de 8 casas (o código agora só
-- grava assim). Perda máxima: 5e-9 BRL por valor — abaixo de qualquer centavo.
-- O livro-razão (transactions) fica INTACTO: é histórico imutável e todos os
-- seus valores foram gravados via Decimal (logo, são representáveis na volta).
UPDATE assets SET unit_value = ROUND(unit_value, 8) WHERE scale(unit_value) > 8;
UPDATE holdings SET avg_cost = ROUND(avg_cost, 8) WHERE scale(avg_cost) > 8;
UPDATE users SET balance = ROUND(balance, 8) WHERE scale(balance) > 8;
UPDATE portfolio_snapshots SET total_value = ROUND(total_value, 8) WHERE scale(total_value) > 8;
