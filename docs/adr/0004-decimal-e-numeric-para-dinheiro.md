# ADR-0004: `Decimal` ↔ `NUMERIC` com escala canônica de 8 casas

## Status

Aceita. Substitui a modelagem inicial em `DOUBLE PRECISION`, revogada pela migração
`20260613000000_money_to_numeric`.

**Este é o ADR mais consequente do projeto**, e o único cuja fundamentação inclui
um incidente de produção documentado.

## Contexto

O schema inicial modelava `assets.unit_value` como `DOUBLE PRECISION`, seguindo o
material didático. Todo valor monetário do sistema — saldo, preço, quantidade,
custo médio, movimentação de caixa, valor de patrimônio — deriva desse tipo ou
interage com ele.

Dois problemas surgiram, em momentos diferentes:

**Problema 1 (2026-06-13).** Ponto flutuante binário não representa exatamente
decimais como 0,1: a soma `0,1 + 0,2` não dá `0,3`. Num sistema onde saldos são
somados, produtos preço×quantidade são calculados e custo médio ponderado é
recalculado a cada compra, o erro acumula e o saldo exibido deixa de bater com a
soma do extrato.

**Problema 2 (2026-07-22) — incidente de produção.** Após a migração para
`NUMERIC`, `/assets` passou a responder **500 para qualquer conta com posições**. A
causa: a sincronização de cotações grava `preço = 1/taxa`, e a divisão de
`rust_decimal::Decimal` preenche a **mantissa inteira** — uma dízima como 1/3 vira
uma dízima de 28 casas. `NUMERIC` do Postgres é de precisão **ilimitada** e aceitou
o valor sem reclamar. Mas `Decimal` tem **28 dígitos significativos**, e um preço
com 28 casas, embora caiba individualmente, faz o **produto ou a soma** dele com
outro valor estourar o limite. Exatamente o que `wallet_summary`, `list_holdings` e
`record_portfolio_snapshots` fazem. A leitura de volta falhava com `value not
representable`.

O detalhe que torna o incidente instrutivo: **cada coluna estava dentro do
invariante**. O estouro acontecia no agregado, na leitura — não na escrita.

## Restrições

- Valores monetários incluem criptomoedas: 2 casas decimais são insuficientes (BTC
  tem 8).
- O tipo do lado Rust precisa integrar com `sqlx` (persistência) e `serde`
  (serialização JSON da API).
- A serialização JSON não pode emitir número de ponto flutuante — um `f64` no meio
  do caminho anularia o resto da decisão.
- Há dados já gravados em produção fora do invariante, que precisam ser saneados.
- A CoinGecko devolve número JSON (`f64`), então existe uma fronteira em que a
  conversão é inevitável.

## Opções consideradas

**Avaliadas de fato:**

1. **`DOUBLE PRECISION` / `f64`** — estado inicial, **revogado por migração**.
2. **`NUMERIC` + `rust_decimal::Decimal` sem escala canônica** — estado
   intermediário, **causou o incidente**.
3. **`NUMERIC` + `Decimal` com escala canônica de 8 casas na escrita e `ROUND` na
   leitura** — decisão atual.

**Comparação *post hoc***:

4. Inteiro de centavos (`i64`).
5. `bigdecimal` (precisão arbitrária, sem teto de 28 dígitos).

## Decisão

`rust_decimal::Decimal` ↔ `NUMERIC`, com **`MONEY_SCALE = 8` como invariante em
duas pontas**:

- **Escrita:** todo valor monetário é arredondado para 8 casas antes de chegar ao
  banco.
- **Leitura:** todo agregado SQL que soma ou multiplica `NUMERIC` é envolvido em
  `ROUND(..., 8)`.

`f64` é permitido **exclusivamente** em coordenadas de desenho SVG, que não são
dinheiro.

## Fundamentação

**Motivo confirmado** para abandonar ponto flutuante — a própria migração nomeia:

> "Money must not be stored as floating point: `DOUBLE PRECISION` carries rounding
> noise (e.g. 0.1 + 0.2 != 0.3) that is unacceptable for financial values."

**Motivo confirmado** para a escala canônica: o comentário de `MONEY_SCALE` e a
mensagem da migração de saneamento descrevem o mecanismo do incidente em detalhe.

A correção tem **três camadas**, e a existência de três não é redundância:

1. **Escrita arredonda sempre.** `brl_price` arredonda a inversão da taxa;
   `validated_unit_value` faz o mesmo em qualquer escrita administrativa;
   `buy_asset`/`sell_asset` arredondam o produto preço×quantidade.
2. **Leitura envolve agregados em `ROUND(..., 8)`.** Esta é a camada que parece
   redundante e **não é**: produtos e somas de `NUMERIC` acumulam escala sem
   limite, então a leitura falharia mesmo com cada coluna individual dentro do
   invariante. O comentário no repository nomeia isso explicitamente.
3. **A migração `normalize_money_scales` saneou o estado gravado**, arredondando
   `unit_value`, `avg_cost`, `balance` e `total_value` acima de 8 casas. Perda
   máxima registrada: 5×10⁻⁹ BRL por valor — abaixo de qualquer centavo.

`transactions` foi **deliberadamente não tocada** pela migração: é histórico
imutável, e todos os seus valores foram gravados via `Decimal`, logo já são
representáveis na volta.

E existe um **teste de regressão nomeado pelo incidente**:
`legacy_high_scale_money_still_renders_the_wallet` planta deliberadamente valores
de 28 casas no banco, simulando o estado anterior, e confirma que toda leitura
(`wallet_summary`, `list_holdings`, snapshot, nova compra) continua decodificando.
O teste existe especificamente para que esta classe de bug não volte.

**Defesa em profundidade no schema.** A migração `financial_guardrails` adiciona
`CHECK (unit_value >= 0)` em `assets` e `CHECK (quantity IS NULL OR quantity > 0)`
em `transactions`, mesmo essas condições já sendo validadas em Rust. O comentário é
explícito: "a aplicação já valida isso na borda HTTP; o banco é a última linha de
defesa: nenhum caminho de escrita — API do admin, sincronização de cotação, SQL
manual — consegue persistir um valor inválido."

**Serialização como string.** `Decimal` serializa como string JSON, não como número.
O teste `the_catalogue_round_trips_through_real_http_requests` confirma que
"dinheiro sai como string JSON" — porque um `f64` no meio do caminho seria
exatamente o que o projeto evita de ponta a ponta.

## Consequências positivas

- Aritmética financeira exata de ponta a ponta, verificada por 26 testes contra
  Postgres real.
- Custo médio ponderado confiável: `(2×10 + 2×20) / 4 = 15` exatamente.
- O schema recusa valores inválidos independentemente do caminho de escrita.
- 8 casas cobrem cripto com folga.
- A API não pode vazar imprecisão: dinheiro é string no JSON.

## Consequências negativas

- **O teto de 28 dígitos significativos é real**, não teórico. Qualquer query nova
  que some ou multiplique dinheiro **precisa** do `ROUND`, e nada no compilador
  força isso — é disciplina sustentada por comentário e por teste de regressão.
- **Aritmética mais lenta** que ponto flutuante nativo. Irrelevante nesta escala,
  mas presente.
- **Impedância na fronteira com terceiros.** `from_f64_retain` traz o erro de
  representação binária (0,1 vira 0,1000000000000000055…), então
  `decimal_from_f64` precisa travar a escala explicitamente.
- **`MONEY_SCALE = 8` é uma escolha, não uma lei.** Um ativo com precisão maior
  exigiria revisar o invariante inteiro, as migrações e os `ROUND`.
- Operações cujo total arredonda a zero precisam ser **recusadas explicitamente**
  (`AppError::TradeTooSmall`), senão viram um moedor de unidades grátis.
- Perda de precisão irreversível de até 5×10⁻⁹ BRL nos dados saneados pela
  migração.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Query nova de agregado sem `ROUND` | **Alto** — 500 na tela da carteira, o incidente repetido | Comentário no repository; teste de regressão; revisão de código. **Não há verificação automática** |
| Caminho de escrita novo sem `round_dp` | Alto | `CHECK` no schema pega valor negativo, **mas não escala excessiva** |
| Ativo exigir mais de 8 casas | Médio | Nenhuma — exigiria revisão do invariante |
| `rust_decimal` abandonado | Médio | `bigdecimal` é substituto identificado |

## Evidências

```text
- src/models.rs                                     · MONEY_SCALE (com o porquê)
- src/quotes.rs                                     · brl_price (round_dp obrigatório)
- src/repository.rs                                 · validated_unit_value, buy_asset,
                                                      sell_asset, wallet_summary,
                                                      list_holdings,
                                                      record_portfolio_snapshots
- src/market.rs                                     · decimal_from_f64
- migrations/20260613000000_money_to_numeric.up.sql
- migrations/20260716000000_financial_guardrails.up.sql
- migrations/20260722000000_normalize_money_scales.up.sql
- testes: legacy_high_scale_money_still_renders_the_wallet,
          deposits_and_trades_reject_excessive_scale,
          brl_price_caps_the_scale_of_non_terminating_inversions,
          trades_that_round_to_zero_do_not_move_cash_or_holdings,
          admin_prices_are_capped_at_the_canonical_scale,
          every_decimal_from_the_payload_arrives_with_its_scale_capped,
          inverting_real_rates_never_leaks_a_scale_the_database_cannot_take_back
```

## Critérios de revisão

Reavaliar se:

1. Um ativo exigir mais de 8 casas decimais.
2. Algum agregado passar a encadear multiplicações a ponto de 28 dígitos
   significativos ficarem apertados mesmo com `ROUND` — nesse caso, `bigdecimal`
   é a substituição natural e resolve o problema na raiz, ao custo da integração
   direta com `sqlx`/`serde`.
3. O `rust_decimal` deixar de ser mantido.

**Recomendação de reforço, não implementada:** um teste que percorra todos os
métodos de escrita do `Repository` e confirme que nenhum grava escala acima de
`MONEY_SCALE` fecharia o risco de "caminho de escrita novo sem `round_dp`", que
hoje depende só de revisão humana.
