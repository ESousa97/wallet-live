# ADR-0005: `holdings` materializado + `transactions` como livro-razão imutável

## Status

Aceita. Substitui a modelagem `owned_assets`, removida pela migração
`20260613000002_holdings_and_transactions`.

## Contexto

O material didático modelava `owned_assets` como um **log de compras append-only**
e derivava tudo por agregação em tempo de leitura: quantidade possuída era
`SUM(quantity)`, custo médio era `SUM(quantity × unit_value) / SUM(quantity)`.

Isso funciona para um produto que **só compra**. O `wallet-live` também vende, e a
venda quebra o modelo em dois pontos:

1. **Não há como representar uma saída.** Uma linha com quantidade negativa
   funcionaria para a soma, mas envenenaria o cálculo de custo médio — o
   denominador passaria a subtrair.
2. **A agregação a cada leitura** cresce com o histórico. Uma conta com mil
   operações recalcularia mil linhas em cada carregamento da carteira.

Havia também um requisito de produto que o modelo antigo não atendia: **extrato
auditável** com depósitos, compras e vendas em ordem cronológica, paginado e
exportável em CSV. `owned_assets` só conhecia compras.

## Restrições

- Migração de dados existentes sem perda: havia registros de `owned_assets` em uso.
- Operações monetárias precisam ser transacionais e atômicas.
- O extrato precisa ser imutável para servir de auditoria.
- Postgres como único armazenamento; sem event store nem CQRS.

## Opções consideradas

**Avaliadas de fato** (a mensagem da migração descreve a deliberação):

1. **Manter `owned_assets` append-only** e representar venda como quantidade
   negativa — envenena o custo médio.
2. **Separar em duas tabelas**: posição materializada + livro-razão imutável —
   decisão adotada.
3. Manter uma só tabela e recalcular tudo por agregação, tratando venda com uma
   coluna de sinal.

**Comparação *post hoc***: event sourcing puro (só o log, com projeções
materializadas por processo separado).

## Decisão

Duas tabelas com responsabilidades distintas:

- **`holdings`** — a posição **atual** por `(user_id, asset_id)`: quanto se possui
  e o custo médio. Chave primária **composta**, uma linha por posição, mutada
  atomicamente na compra e na venda. A linha é **apagada**, não zerada, quando a
  posição fecha.
- **`transactions`** — o livro-razão **imutável** de tudo que aconteceu. `kind`
  restrito por `CHECK` a `'deposit'`/`'buy'`/`'sell'`, e `cash_delta` **assinado**
  (depósito positivo, compra negativa, venda positiva).

## Fundamentação

**Motivo confirmado.** O comentário no topo da migração é a explicação mais direta
de uma decisão de arquitetura em todo o repositório:

> "O curso modelou `owned_assets` como um log de compras *append-only* e derivava
> tudo (quantidade possuída, lucro/prejuízo) agregando isso em tempo de leitura.
> Isso funciona para só-compra, mas uma carteira de verdade também vende, então
> separamos a preocupação em duas: `holdings` (a posição atual por usuário/ativo —
> quanto se possui e o custo médio, mutada atomicamente na compra/venda) e
> `transactions` (o livro-razão imutável de tudo que aconteceu, para o histórico e
> auditoria)."

O ganho prático é mensurável: `wallet_summary` e `list_holdings` são consultas
**triviais** — um `JOIN`, sem agregação pesada — porque a agregação já aconteceu no
momento da **escrita**. Leituras triviais, escrita explícita.

**Decisões de detalhe que merecem registro:**

- **Apagar a linha em vez de zerar.** Uma posição fechada tem de sair da carteira,
  não ficar com quantidade 0 poluindo a tela. Travado por
  `selling_everything_closes_the_position`.
- **Venda parcial não recalcula o custo médio.** Recalcular na venda **inventaria
  lucro**: o custo médio se refere às unidades que **permanecem**, e vender parte
  não muda o que foi pago pelas restantes. Travado por
  `partial_sell_keeps_remaining_units`.
- **`cash_delta` assinado em vez de coluna de tipo + valor absoluto.** Permite que
  a soma do extrato seja diretamente conferível contra o saldo.
- **`quantity` e `unit_value` são `NULL` para depósito.** Depósito não envolve
  ativo; usar zero seria um valor falso.
- **Índice `(user_id, created_at DESC)`.** A view de histórico sempre filtra por
  usuário e ordena por recência — o índice espelha exatamente o padrão de acesso.

**A migração preserva os dados.** Dois `INSERT ... SELECT`: um agrega
`owned_assets` para popular `holdings`, outro reconstitui `transactions` a partir
do mesmo histórico, marcando tudo como `'buy'` com `cash_delta` negativo. Só então
`owned_assets` é removida.

## Consequências positivas

- Venda passa a ser representável, com custo médio correto.
- Leitura da carteira é O(posições), não O(histórico).
- Extrato imutável serve de auditoria e de fonte do CSV.
- `CHECK (quantity >= 0)` e `CHECK (avg_cost >= 0)` no schema como última linha de
  defesa.
- Chave primária composta impede duas linhas para a mesma posição.

## Consequências negativas

- **Duas fontes de verdade que podem divergir.** Se um caminho de escrita atualizar
  `holdings` sem inserir em `transactions` (ou vice-versa), o extrato deixa de
  bater com a posição. É por isso que **toda** operação roda em transação — mas a
  consistência depende de disciplina no código, não de constraint.
- **Não há verificação de reconciliação.** Nada confere periodicamente que
  `holdings.quantity` corresponde à soma das transações do par usuário/ativo.
  Registrado como débito técnico.
- **Escrita mais complexa.** `buy_asset` faz `SELECT ... FOR UPDATE`, calcula custo
  médio, atualiza saldo, faz `UPSERT` em `holdings` e insere em `transactions` —
  cinco passos numa transação, contra um `INSERT` no modelo antigo.
- **Custo médio é destrutivo.** Ao contrário do log append-only, o valor anterior
  do custo médio não é recuperável de `holdings` — só reconstituível a partir de
  `transactions`.
- A migração para `holdings` assume que todo registro de `owned_assets` era compra;
  não havia venda no modelo antigo, então a premissa vale, mas é uma premissa.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| `holdings` divergir de `transactions` | **Alto** — posição exibida não corresponde ao histórico | Transação em toda operação; 26 testes contra Postgres real. **Nenhuma reconciliação automática** |
| Caminho de escrita novo esquecer o livro-razão | Alto | Revisão de código; `deposit_credits_balance_and_logs_transaction` trava que as duas escritas são uma transação |
| Custo médio corrompido por venda | Médio | `partial_sell_keeps_remaining_units` |
| Venda a descoberto | Alto | `CHECK (quantity >= 0)` + `sell_rejects_more_than_owned` |

## Evidências

```text
- migrations/20260613000002_holdings_and_transactions.up.sql  (a decisão, comentada)
- migrations/20260716000000_financial_guardrails.up.sql        (CHECKs)
- src/repository.rs · buy_asset, sell_asset, deposit,
                      wallet_summary, list_holdings,
                      list_transactions, count_transactions,
                      list_all_transactions
- src/models.rs     · Holding, Transaction, WalletSummary
- testes: deposit_credits_balance_and_logs_transaction,
          buy_debits_balance_and_opens_holding,
          buying_more_averages_the_cost_basis,
          selling_everything_closes_the_position,
          partial_sell_keeps_remaining_units,
          sell_rejects_more_than_owned,
          buy_rejects_when_balance_is_insufficient,
          transactions_paginate_newest_first_without_gaps
```

## Critérios de revisão

Reavaliar se:

1. Surgir requisito de reconstruir o estado da carteira em qualquer instante
   passado (auditoria retroativa) — aí event sourcing puro passa a fazer sentido, e
   `holdings` viraria projeção descartável.
2. A divergência entre `holdings` e `transactions` ocorrer de fato em produção — o
   que indicaria que a garantia por transação não está bastando.
3. O volume de posições por usuário crescer a ponto de a leitura de `holdings`
   deixar de ser trivial.

**Recomendação não implementada:** uma consulta de reconciliação — comparar
`holdings.quantity` com a soma assinada de `transactions.quantity` por par — que
possa ser executada sob demanda ou por job periódico. Registrada em
[../decisions/technical-debt.md](../decisions/technical-debt.md).
