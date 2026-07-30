# Migrações

## Objetivo

Documentar as 11 migrações do projeto — o que cada uma resolveu, sua
reversibilidade e o procedimento para criar novas — e registrar os riscos do
mecanismo de aplicação automática no boot.

## Escopo

Coberto: histórico, mecanismo de aplicação, reversibilidade, procedimento
operacional. Não coberto: o schema resultante (ver
[database-schema.md](database-schema.md)).

---

## 1. Mecanismo

As migrações são **embutidas no binário** por `sqlx::migrate!()`, que lê o diretório
`migrations/` em tempo de compilação, e aplicadas **no boot**, dentro de
`AppState::build`, antes de o serviço aceitar qualquer requisição.

| Propriedade | Comportamento |
| --- | --- |
| Idempotência | Migração já aplicada é pulada (controle em `_sqlx_migrations`) |
| Falha | **Aborta o boot** — o serviço não sobe |
| Ordem | Lexicográfica pelo prefixo de timestamp |
| Passo manual no deploy | **Nenhum** |

A escolha de abortar o boot é deliberada, e o comentário no código a justifica:
"melhor não subir do que subir contra um schema pela metade."

**Consequência operacional que precisa estar clara:** um deploy com migração ruim
**derruba o serviço** em vez de subi-lo degradado. O procedimento de recuperação está
em [../operations/runbooks.md](../operations/runbooks.md).

## 2. Histórico

As 11 migrações não são um schema desenhado de uma vez — são uma sequência real de
decisões, cada uma resolvendo um problema que a anterior não previa.

| # | Data | Migração | O que resolveu |
| --: | --- | --- | --- |
| 1 | 2026-06-02 | `create_assets` | Catálogo de ativos. `unit_value` nasceu `DOUBLE PRECISION` |
| 2 | 2026-06-03 | `create_users` | `username` único, `password_hash` — nunca senha em texto |
| 3 | 2026-06-04 | `create_owned_assets` | Histórico de compras append-only |
| 4 | 2026-06-13 | `money_to_numeric` | **`DOUBLE PRECISION` → `NUMERIC`**, por ruído de arredondamento |
| 5 | 2026-06-13 | `add_user_balance` | Saldo em caixa + `CHECK` de não negativo |
| 6 | 2026-06-13 | `holdings_and_transactions` | **Reformulação central do domínio** |
| 7 | 2026-07-16 | `financial_guardrails` | `CHECK` de preço/quantidade no schema |
| 8 | 2026-07-16 | `create_sessions` | Tabela de sessões para o refresh token |
| 9 | 2026-07-17 | `user_roles` | Coluna `role` com `DEFAULT 'user'` |
| 10 | 2026-07-18 | `portfolio_snapshots` | Série temporal do patrimônio |
| 11 | 2026-07-22 | `normalize_money_scales` | **Correção de um incidente de produção** |

Três merecem detalhamento.

### #6 — `holdings_and_transactions` (reformulação de domínio)

Substituiu `owned_assets` por duas tabelas com responsabilidades distintas. O motivo
está escrito na própria migração: o modelo append-only funciona para um produto que
só compra, mas uma carteira real também vende.

**A migração preserva os dados**: dois `INSERT ... SELECT` — um agrega `owned_assets`
para popular `holdings` (somando quantidades e calculando o custo médio), outro
reconstitui `transactions` marcando tudo como `'buy'` com `cash_delta` negativo. Só
então `owned_assets` é removida.

Detalhe: a reconstituição assume que **todo** registro de `owned_assets` era compra.
A premissa vale porque não havia venda no modelo antigo — mas é uma premissa, não uma
dedução.

Ver [ADR-0005](../adr/0005-holdings-materializados-e-livro-razao.md).

### #7 — `financial_guardrails` (defesa em profundidade)

Adiciona `CHECK` que **já eram validados em Rust**. A duplicação é intencional, e a
migração explica: "a aplicação já valida isso na borda HTTP; o banco é a última linha
de defesa: nenhum caminho de escrita — API do admin, sincronização de cotação, SQL
manual — consegue persistir um valor inválido."

Dois lugares diferentes concordando que "preço negativo" nunca é estado válido, um
deles impossível de contornar mesmo por um bug na camada Rust.

### #11 — `normalize_money_scales` (saneamento de incidente)

**A única migração de correção de dado do projeto.** A sincronização de cotações
gravava `preço = 1/taxa` sem arredondar, e o `NUMERIC` ficava com até 28 casas.
Produtos e somas sobre esses valores estouravam os 28 dígitos significativos do
`Decimal`, e a **leitura** falhava — `/assets` respondia 500 para qualquer conta com
posições.

```sql
UPDATE assets              SET unit_value  = ROUND(unit_value, 8)  WHERE scale(unit_value)  > 8;
UPDATE holdings            SET avg_cost    = ROUND(avg_cost, 8)    WHERE scale(avg_cost)    > 8;
UPDATE users               SET balance     = ROUND(balance, 8)     WHERE scale(balance)     > 8;
UPDATE portfolio_snapshots SET total_value = ROUND(total_value, 8) WHERE scale(total_value) > 8;
```

Duas propriedades desta migração merecem registro:

1. **Perda máxima de 5×10⁻⁹ BRL por valor** — abaixo de qualquer centavo, declarada
   na própria migração.
2. **`transactions` fica intacta**, deliberadamente: é histórico imutável, e todos os
   seus valores foram gravados via `Decimal`, logo já são representáveis.

Ver [ADR-0004](../adr/0004-decimal-e-numeric-para-dinheiro.md).

## 3. Reversibilidade

Todas as 11 têm arquivo `.down.sql`. **Nenhuma é executada por teste** — a
reversibilidade é afirmada por construção, não verificada.

| Migração | Reversão | Perde dado? |
| --- | --- | :---: |
| 1, 2, 3, 8, 10 | `DROP TABLE` | **Sim** — a tabela inteira |
| 4 (`money_to_numeric`) | `NUMERIC` → `DOUBLE PRECISION` | **Sim** — precisão |
| 5, 9 | `DROP COLUMN` | **Sim** — a coluna |
| 6 | Recria `owned_assets` | **Sim** — vendas não são representáveis no modelo antigo |
| 7 | `DROP CONSTRAINT` | Não |
| 11 | *(vazia)* | — |

Três alertas concretos:

> **A reversão da #4 é destrutiva de precisão.** Voltar `NUMERIC` para `DOUBLE
> PRECISION` reintroduz o erro de ponto flutuante em todos os valores já gravados, de
> forma irreversível.

> **A reversão da #6 perde informação por natureza.** `owned_assets` não representa
> vendas; qualquer venda registrada depois da migração não tem para onde voltar.

> **A #11 não tem reversão possível**, e o arquivo `.down.sql` é intencionalmente
> vazio: arredondamento não é reversível — não existe informação para restaurar as
> casas descartadas.

## 4. Criar uma migração nova

Requer o `sqlx-cli`:

```bash
cargo install sqlx-cli --no-default-features --features postgres,rustls
```

Com o banco de pé (`docker compose up -d db`):

```bash
cargo sqlx migrate add -r nome_descritivo
```

O `-r` cria o par `up`/`down`. Depois de escrever o SQL:

```bash
cargo sqlx migrate run
```

Se a migração alterar qualquer coisa que as queries usem, o cache offline precisa ser
regenerado — senão o CI falha:

```bash
cargo sqlx prepare
```

### Lista de verificação antes de commitar

| Item | Por quê |
| --- | --- |
| O `.down.sql` foi escrito e revisado? | O `-r` cria o arquivo, não o conteúdo |
| Perda de dado na reversão está documentada? | Como nas #4, #6 e #11 acima |
| Comentário no topo explica o **motivo**? | É a convenção do projeto, e é o que torna estas migrações legíveis |
| `cargo sqlx prepare` foi executado? | Cache desatualizado quebra o CI |
| A migração é idempotente onde possível? | `IF NOT EXISTS` / `IF EXISTS` |
| Migração de dado usa `WHERE` seletivo? | A #11 usa `WHERE scale(...) > 8` para não reescrever linhas corretas |
| Valores monetários novos respeitam `MONEY_SCALE`? | Ver [ADR-0004](../adr/0004-decimal-e-numeric-para-dinheiro.md) |
| Agregados novos têm `ROUND(..., 8)`? | Sem isso, a leitura pode estourar o `Decimal` |

## 5. Riscos do mecanismo

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Migração falha no boot | **Alto** — serviço não sobe | Deliberado. **Nenhum teste executa as migrações em sequência** |
| `.down.sql` nunca testado | Médio — rollback pode falhar quando mais necessário | **Nenhuma** |
| Cache `.sqlx` descolar do schema | Médio | `cargo sqlx prepare --check` no CI |
| Migração longa travando o boot | Médio — sem timeout nem indicação de progresso | Nenhuma |
| Múltiplas instâncias migrando ao mesmo tempo | Baixo | O `sqlx` usa lock consultivo do Postgres |
| Migração de dado sem `WHERE` seletivo | Médio | Convenção, revisão de código |

**Recomendação não implementada:** um teste que aplique todas as migrações e as
reverta em sequência fecharia os dois primeiros riscos. Registrado em
[../decisions/technical-debt.md](../decisions/technical-debt.md).

## 6. Evidências

```text
- migrations/                  (11 pares up/down)
- src/app.rs                   · AppState::build (sqlx::migrate!().run)
- .sqlx/                       (31 queries verificadas contra o schema)
- .github/workflows/ci.yml     (cargo sqlx migrate run; cargo sqlx prepare --check)
- .cargo/config.toml           (SQLX_OFFLINE)
```
