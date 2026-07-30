# Débitos técnicos

## Objetivo

Registro consolidado dos débitos técnicos identificados: origem, impacto, risco,
prioridade e recomendação. É o índice canônico dos identificadores `DT-XX` citados no
restante da documentação.

## Escopo

Coberto: débitos — itens que **deveriam** ser corrigidos. Não coberto: limitações
conscientes que não se pretende corrigir (ver
[known-limitations.md](known-limitations.md)) e riscos de segurança com análise de
ameaça (ver [../security/threat-model.md](../security/threat-model.md)).

---

## Taxonomia

Distinguir a natureza de cada item importa, porque o tratamento é diferente:

| Categoria | Definição | Tratamento |
| --- | --- | --- |
| **Bug** | Comportamento incorreto | Corrigir |
| **Débito técnico** | Solução que funciona, mas com custo futuro | Priorizar |
| **Decisão temporária** | Escolha adequada ao estágio atual | Revisar no gatilho |
| **Ausência de teste** | Comportamento não verificado | Escrever |
| **Ausência de observabilidade** | Sinal que não existe | Instrumentar |
| **Risco de segurança** | Ver o modelo de ameaças | Priorizar por impacto |
| **Restrição operacional** | Limite de operação | Documentar e planejar |
| **Limitação conhecida** | Consequência aceita de uma decisão | **Não** é débito — ver o outro documento |

## Resumo por prioridade

| Prioridade | Quantidade | IDs |
| --- | ---: | --- |
| **Alta** | 7 | DT-04, DT-05, DT-07, DT-09, DT-10, DT-12, DT-23 |
| Média | 11 | DT-01, DT-02, DT-03, DT-06, DT-11, DT-13, DT-15, DT-17, DT-20, DT-21, DT-22 |
| Baixa | 6 | DT-08, DT-14, DT-16, DT-18, DT-19, DT-24 |

---

## Registro

| ID | Débito ou limitação | Categoria | Origem | Impacto | Risco | Prior. | Recomendação |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **DT-01** | `LoginThrottle` guarda estado **em memória** | Restrição operacional | `src/auth/throttle.rs` | Lockout por instância; reinício zera todos os bloqueios | Força bruta mais viável com N réplicas ou reinícios frequentes | Média | Mover para armazenamento compartilhado **se** houver mais de uma instância |
| **DT-02** | `sessions` cresce **sem expurgo** | Débito técnico | `migrations/…create_sessions` | ~6 linhas/hora por sessão ativa, para sempre | Degradação lenta; consumo de disco | Média | `DELETE` periódico de revogadas e expiradas |
| **DT-03** | `portfolio_snapshots` cresce **sem expurgo** | Débito técnico | `migrations/…portfolio_snapshots` | **144 linhas/usuário/dia**; o gráfico lê só os últimos 60 pontos | Crescimento proporcional ao **tempo**, não ao uso | Média | Expurgo ou agregação do histórico antigo |
| **DT-04** | `COOKIE_SECURE` comparado **literalmente** com `"true"` | **Bug** | `src/config.rs` | `TRUE`, `1`, `yes` resultam em `false`, **silenciosamente** | **Cookies de sessão em HTTP claro num ambiente que se acredita protegido** | **Alta** | Aceitar `true`/`1`/`yes` sem distinção de caixa, com `trim` |
| **DT-05** | **Nenhum backup implementado** | Restrição operacional | Ausente | `docker compose down -v` ou falha de disco = **perda total e irreversível** | `transactions` é o livro-razão insubstituível | **Alta** | `pg_dump` diário fora do host; **e backup antes de toda migração** |
| **DT-06** | Nome de ativo duplicado vira **500** | **Bug** | `src/repository.rs` · `create_asset` | Erro do cliente reportado como erro interno | Baixo, mas contradiz o padrão do projeto | Média | Traduzir a violação de `UNIQUE` para `400`, como `add_user` já faz com `UsernameTaken` |
| **DT-07** | Segredos validados só por **presença**, não qualidade | **Risco de segurança** | `src/config.rs` · `required` | `JWT_SECRET=a` é aceito; `change-me` do exemplo também | **Crítico se explorado** — permite forjar sessão de admin | **Alta** | Exigir comprimento mínimo (32+) e recusar valores de exemplo conhecidos |
| **DT-08** | **Sem ferramenta de cobertura** | Ausência de teste | Ausente | Percentual real desconhecido; lacunas invisíveis | Código sem teste não identificado | Baixa | `cargo-llvm-cov`, medindo **ramo**, sem limiar inicial |
| **DT-09** | As 11 migrações `.down.sql` **nunca são executadas** | Ausência de teste | `migrations/` | Reversibilidade afirmada, não verificada | **Rollback pode falhar no pior momento** | **Alta** | Teste que aplique e reverta todas em sequência |
| **DT-10** | `config.rs` **sem teste direto** | Ausência de teste | `src/config.rs` | A validação *fail-fast* não é verificada | Serviço poderia subir sem validar segredo | **Alta** | Extrair a validação para função pura que receba um mapa, e testá-la |
| **DT-11** | Sem reconciliação `holdings` × `transactions` | Débito técnico | [ADR-0005](../adr/0005-holdings-materializados-e-livro-razao.md) | Divergência entre posição e histórico não seria detectada | Posição exibida não corresponde ao livro-razão | Média | Consulta de reconciliação sob demanda ou periódica |
| **DT-12** | Escala monetária **não garantida pelo schema** | **Risco de segurança** | `NUMERIC` sem precisão declarada | `INSERT` manual pode gravar 28 casas | **Reproduz o incidente de 2026-07-22** (500 na carteira) | **Alta** | `NUMERIC(38, 8)` nas colunas monetárias |
| **DT-13** | Sem auditoria de alteração de preço | Ausência de observabilidade | Ausente | Não se sabe **quem** alterou `unit_value`, nem o valor anterior | Alteração indevida é difícil de atribuir | Média | Tabela de auditoria com autor, valor anterior e novo |
| **DT-14** | Promoção a admin exige `UPDATE` manual | Restrição operacional | `src/repository.rs` · `set_user_role` | Nenhuma rota expõe a função | Atrito operacional; seguro por omissão | Baixa | Rota administrativa protegida |
| **DT-15** | htmx vendorado **fora do `cargo audit`** | Risco de segurança | `static/htmx.js` (2.0.8) | Vulnerabilidade em JS não é detectada | Médio | Registrar a versão em local verificável por ferramenta |
| **DT-16** | Versão do Tailwind CLI fixada em **dois lugares** | Débito técnico | `.github/workflows/ci.yml` + máquina local | Divergência produz `diff` espúrio | Falso negativo no CI | Baixa | Fixar num arquivo lido pelos dois |
| **DT-17** | Versão **nunca incrementada**; sem tags | Débito técnico | `Cargo.toml` = `0.1.0` em 36 commits | Não há como referenciar uma versão | Rollback e relatório sem ponto de retorno | Média | Adotar SemVer e criar tags |
| **DT-18** | Sem header `Retry-After` em `429`/`503` | Débito técnico | `src/error.rs`, `src/app.rs` | Cliente não sabe quanto esperar | Baixo | Acrescentar o header |
| **DT-19** | Operações financeiras **não idempotentes** | Débito técnico | `src/routes/frontend.rs` | Dois `POST /deposit` creditam duas vezes | Duplicação por retentativa ou duplo clique | Baixa | Chave de idempotência |
| **DT-20** | RUSTSEC-2023-0071 sem **data de reavaliação** | Débito técnico | `.cargo/audit.toml` | Ignorado indefinidamente | Baixo neste uso (só HS256) | Média | Registrar data de revisão; avaliar trocar `jwt-simple` |
| **DT-21** | `cargo audit` só em **push e PR** | Ausência de observabilidade | `.github/workflows/ci.yml` | Advisory publicado em período sem commits passa despercebido | Médio | Execução agendada (`schedule:`) |
| **DT-22** | **Sem métrica de negócio** | Ausência de observabilidade | `src/app.rs` | Falha de sincronização de cotação **só aparece em log** | **Preços congelados sem alerta** — e eles lastreiam operações | Média | Contador de falhas de sincronização e de erros por tipo |
| **DT-23** | `DATABASE_URL` completa vai para o log em erro de conexão | **Risco de segurança** | `src/app.rs` · boot | **Senha do banco no log** | Log é ativo sensível e costuma ter retenção longa | **Alta** | Sanitizar a credencial antes de registrar |
| **DT-24** | Sem política de descontinuação da API | Débito técnico | [ADR-0011](../adr/0011-versionamento-da-api-por-caminho.md) | Destino do alias `/api` indefinido quando o v2 existir | Consumidor preso sem aviso | Baixa | Definir **antes** da primeira mudança incompatível |

---

## Ações prioritárias

Ordenadas por (impacto × probabilidade) ÷ esforço. **As três primeiras são de baixo
esforço e alto retorno.**

| # | Ação | Débito | Esforço |
| --- | --- | --- | --- |
| 1 | Corrigir o parsing de `COOKIE_SECURE` | DT-04 | **Baixo** |
| 2 | Exigir comprimento mínimo e recusar valores de exemplo nos segredos | DT-07 | **Baixo** |
| 3 | Sanitizar a `DATABASE_URL` no log | DT-23 | **Baixo** |
| 4 | `pg_dump` antes de toda migração | DT-05 | Baixo |
| 5 | Backup diário automatizado, fora do host | DT-05 | Médio |
| 6 | Teste de aplicação e reversão das migrações | DT-09 | Médio |
| 7 | Tornar `Config` testável e escrever o teste | DT-10 | Médio |
| 8 | `NUMERIC(38, 8)` nas colunas monetárias | DT-12 | Médio |
| 9 | Contador de falhas de sincronização | DT-22 | Baixo |
| 10 | Expurgo de `sessions` e `portfolio_snapshots` | DT-02, DT-03 | Médio |

> **DT-04, DT-07 e DT-23 compartilham uma característica perigosa: falham em
> silêncio.** Um serviço com `COOKIE_SECURE=TRUE`, `JWT_SECRET=a` e a senha do banco no
> log funciona normalmente — até ser explorado. São os três primeiros da lista por
> isso, não por serem os mais complexos.

## Débitos já quitados

Registro do que **foi** débito e deixou de ser, porque a forma como foram resolvidos é
o padrão esperado para os demais:

| Débito | Como foi resolvido | Evidência |
| --- | --- | --- |
| Dinheiro em `DOUBLE PRECISION` | Migração para `NUMERIC` | `migrations/…money_to_numeric` |
| Escala monetária estourando o `Decimal` | Correção em três camadas + migração de saneamento + **teste de regressão nomeado pelo incidente** | `legacy_high_scale_money_still_renders_the_wallet` |
| `owned_assets` não suportava venda | Reformulação em `holdings` + `transactions`, com migração de dados | [ADR-0005](../adr/0005-holdings-materializados-e-livro-razao.md) |
| Tailwind Play CDN enfraquecendo a CSP | Compilação em build-time | [ADR-0010](../adr/0010-css-compilado-em-build-time.md) |
| JWT longo sem revogação | Access curto + refresh rotativo revogável | [ADR-0007](../adr/0007-sessao-jwt-curto-com-refresh-rotativo.md) |
| Autorização só por secret key | Papel de sessão como caminho preferido | [ADR-0008](../adr/0008-autorizacao-por-papel-e-credencial-de-servico.md) |
| API sem versão | `/api/v1` com alias verificado por teste | [ADR-0011](../adr/0011-versionamento-da-api-por-caminho.md) |
| Sem camada de teste de integração | `src/lib.rs` habilitou `tests/` | [ADR-0001](../adr/0001-rust-como-linguagem-unica.md) |
| Autorização não exercida nos testes | `tests/http_api.rs` pelo router real | `writing_to_the_catalogue_requires_the_admin_credential` |
| Queries N+1 na sincronização | Um `UPDATE` com `UNNEST` | `update_known_asset_prices` |

**O padrão de quitação deste projeto** é visível na segunda linha: correção em
múltiplas camadas, saneamento do dado existente, e um **teste nomeado pelo incidente**
para que a classe de bug não volte.

## Como usar este registro

1. Ao corrigir um débito, **mova a linha** para "Débitos já quitados" com a evidência.
2. Ao identificar um débito novo, use o próximo ID livre e cite-o no documento
   relevante.
3. Não remova um débito sem corrigi-lo — se ele deixar de importar, mova para
   [known-limitations.md](known-limitations.md) com a justificativa.

## Evidências

```text
- src/config.rs          · Config::from_env, required (DT-04, DT-07, DT-10)
- src/repository.rs      · create_asset (DT-06), set_user_role (DT-14)
- src/auth/throttle.rs   · LoginThrottle (DT-01)
- src/app.rs             · RequestMetrics (DT-22), boot (DT-23)
- migrations/            (DT-02, DT-03, DT-09, DT-12)
- .cargo/audit.toml      (DT-20)
- .github/workflows/ci.yml (DT-16, DT-21)
- Cargo.toml             (DT-17)
- static/htmx.js         (DT-15)
```
