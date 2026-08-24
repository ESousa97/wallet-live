# Plano de testes

## Objetivo

Definir objetivos, escopo, ambientes, critérios de entrada e saída, frequência de
execução e gestão de defeitos da atividade de teste.

## Escopo

Coberto: a política de execução — o que roda quando, o que aprova uma mudança, o que
bloqueia. Não coberto: como a suíte é organizada (ver
[test-strategy.md](test-strategy.md)), o inventário caso a caso (ver
[test-catalogue.md](test-catalogue.md)) e a leitura por risco (ver
[test-matrix.md](test-matrix.md)).

> **Contexto de escala.** Este é um projeto de autor único, sem equipe de QA e sem
> ambiente de homologação. O plano descreve o processo **real**, não um processo
> corporativo que não existe. Papéis são todos do mesmo responsável, e isso está
> declarado em vez de disfarçado.

---

## 1. Objetivos

| # | Objetivo | Como é medido |
| --- | --- | --- |
| O1 | Nenhuma operação financeira produz estado inconsistente | 26 testes de repositório contra Postgres real |
| O2 | Nenhum valor monetário perde exatidão | Testes de escala em escrita, leitura e fronteira externa |
| O3 | Nenhuma rota privada é acessível sem sessão | `tests/http_web.rs` sobre o router de produção |
| O4 | Nenhuma escrita administrativa ocorre sem autorização | `tests/http_api.rs` |
| O5 | Mudança de contrato JSON não passa despercebida | 3 snapshots `insta` |
| O6 | Mudança de formato na fonte externa é detectada em CI | 12 testes de contrato contra payload real |
| O7 | A CSP continua fechável (sem inline) | `pages_carry_no_inline_style_or_script` |
| O8 | Direção de variação nunca é comunicada só por cor | `market_dashboard_marks_direction_with_arrow_and_sign_not_only_colour` |

Os objetivos são **ordenados por consequência**: O1 e O2 protegem dinheiro, O3 e O4
protegem acesso, os demais protegem contratos e acessibilidade.

## 2. Itens testados e não testados

### Testados

| Item | Nível | Testes |
| --- | --- | ---: |
| Núcleo financeiro (depósito, compra, venda, custo médio) | Integração com banco real | 26 |
| Renderização e invariantes de HTML | Componente | 18 |
| Fluxo do navegador (sessão, CSRF, dinheiro, mercado) | Contrato HTTP | 15 |
| Snapshot e parse do mercado | Unidade | 11 |
| API administrativa pelo router real | Contrato HTTP | 8 |
| Payload real da CoinGecko | Contrato | 7 |
| Orquestração da carteira | Unidade com dublê | 7 |
| Payload real da Coinbase | Contrato | 5 |
| Idioma, flash, contrato JSON da API | Unidade | 11 |
| CSRF, lockout, validação de cadastro | Unidade | 8 |
| Inversão de taxa e escala | Unidade | 2 |

### **Não** testados — declarados, não omitidos

| Item | Motivo |
| --- | --- |
| Comportamento de JavaScript no DOM | Nada executa JS; só o HTML emitido é verificado |
| Resultado visual e layout responsivo | Só a presença de classes é conferida |
| Chamadas HTTP externas reais (`fetch`) | Teste que bate em API de terceiro mede a internet, não o código |
| Corrida no `RwLock` do mercado e no `Mutex` de cotações | Sem teste de concorrência |
| **Reversão de migrações** | Os 11 `.down.sql` existem e nunca são executados |
| Carga, latência, throughput | Sem medição |
| Instalação e atualização da imagem | O CI prova que compila, não que sobe e serve |
| Recuperação após corrupção de banco | — |
| Perda prolongada de conectividade | — |
| Compatibilidade entre navegadores | — |

## 3. Ambientes

| Ambiente | Onde | Banco | Uso |
| --- | --- | --- | --- |
| Local | Máquina do desenvolvedor | `docker compose up -d db` | Desenvolvimento |
| CI | GitHub Actions (`ubuntu-latest`) | `postgres:18` em service container | Validação de push e PR |
| Efêmero por teste | Dentro de ambos | Criado e destruído por `#[sqlx::test]` | Isolamento |

**Não existe** ambiente de homologação nem de produção. A entrega é reproduzível
localmente e preparada para container; publicar é decisão posterior de
infraestrutura.

### Dependências de ambiente

| Dependência | Necessária para | Alternativa |
| --- | --- | --- |
| PostgreSQL 18 | 100 dos 118 testes | Nenhuma |
| Docker | Subir o Postgres | Instância própria |
| Rust 1.95+ | Tudo | Nenhuma |
| Rede externa | **Nenhum teste** | — |

> A independência de rede é deliberada: os payloads externos são **arquivos
> versionados**, então a suíte roda íntegra sem internet.

## 4. Dados de teste

| Fonte | Onde | Natureza |
| --- | --- | --- |
| Banco efêmero migrado | `#[sqlx::test]` | Vazio, criado por teste |
| Fixtures SQL | `src/routes/fixtures/`, `tests/fixtures/` | Estado inicial mínimo |
| Payloads reais | `tests/payloads/` | Capturas de produção, 2026-07-29 |
| Dublê em memória | `FakeRepository` | Dados construídos no teste |
| Snapshots | `src/routes/snapshots/` | Saída esperada, revisada |

**Nenhum dado pessoal real** é usado. Usuários de teste são `alice`, `bob` e
similares; os valores monetários são construídos para exercitar bordas.

Os payloads reais **contêm cotações reais capturadas** — dado público de mercado, sem
credencial, sem chave, sem informação pessoal. Nenhuma asserção depende do valor: elas
verificam invariantes (escala travada, campo presente, ausente virando neutro).

### Recaptura de payload

Necessária apenas se a fonte mudar de formato. O `User-Agent` não é decoração — a
CoinGecko responde `403` sem ele:

```bash
curl -sS -A "wallet/0.1.0" "https://api.coinbase.com/v2/exchange-rates?currency=BRL" | python -m json.tool > tests/payloads/coinbase_exchange_rates.json
```

## 5. Frequência de execução

| Gatilho | O que roda | Onde | Bloqueia? |
| --- | --- | --- | :---: |
| **A cada commit local** | `cargo test` (recomendado) | Local | Não |
| **Push em `master`** | `lint` + `test` + `audit` + `docker` | CI | **Sim** |
| **Pull request** | Os mesmos 4 jobs | CI | **Sim** |
| **Antes de release** | Os 4 jobs + `build --release` + roteiro manual | Local | **Sim** |
| **Periódico** | *(nada agendado)* | — | — |
| **Manual** | Roteiro de demonstração | Local | Não |
| **Condicional** | `cargo insta review` ao alterar a API | Local | Sim, se houver diff |
| **Destrutivo** | *(nenhum)* | — | — |

> **Não há execução agendada.** O `cargo audit` só roda em push e PR, então uma
> vulnerabilidade publicada num período sem commits passa despercebida até o próximo.
> Registrado como débito técnico.

### Os quatro jobs do CI

| Job | O que faz | Precisa de banco? |
| --- | --- | :---: |
| `lint` | `fmt --check`, `clippy -D warnings`, frescor do CSS compilado | Não (`SQLX_OFFLINE`) |
| `test` | `sqlx migrate run`, `sqlx prepare --check`, `cargo test` | **Sim** |
| `audit` | `cargo audit` (RustSec) | Não |
| `docker` | `docker build .` | Não |

Rodam **em paralelo** e são independentes: uma falha de `audit` não impede saber se
os testes passaram.

## 6. Critérios

### Entrada — o que precisa estar pronto antes de testar

1. O código compila (`cargo build`).
2. Postgres acessível para a suíte completa.
3. Cache `.sqlx/` regenerado, se houve query nova.
4. `static/app.css` recompilado, se houve classe nova.

### Saída — o que caracteriza a atividade como concluída

1. Os 118 testes passam.
2. `cargo fmt --all --check` sem diferenças.
3. `cargo clippy --all-targets -- -D warnings` sem apontamentos.
4. Nenhum snapshot pendente (`*.snap.new`).
5. `cargo sqlx prepare --check` sem divergência.
6. CSS compilado idêntico ao gerado a partir dos templates.

### Aprovação de uma mudança

| Tipo de mudança | Exigência adicional |
| --- | --- |
| Correção de defeito | Teste que **falharia antes** da correção |
| Funcionalidade nova | Teste no nível adequado, com o motivo documentado no catálogo |
| Mudança de contrato JSON | `cargo insta review` **e** avaliação de impacto em consumidores |
| Mudança de schema | Migração `up`/`down` + cache regenerado |
| Mudança em caminho de dinheiro | Teste contra **Postgres real**, não dublê |
| Mudança de template | Verificar `pages_carry_no_inline_style_or_script` |

### Bloqueio — o que impede seguir

1. Qualquer teste falhando.
2. `clippy` com apontamento.
3. Snapshot alterado sem revisão explícita.
4. Cache `.sqlx/` ou CSS desatualizados.
5. `cargo audit` com advisory novo sem justificativa registrada.
6. Mudança em caminho de dinheiro sem teste correspondente.

## 7. Gestão de defeitos

| Etapa | Prática atual |
| --- | --- |
| Registro | GitHub Issues do repositório |
| Classificação | Ver a taxonomia em [../decisions/technical-debt.md](../decisions/technical-debt.md) |
| Correção | Branch a partir de `master`, com teste de regressão |
| Verificação | O teste **falha antes** e passa depois |
| Fechamento | Merge em `master` com CI verde |

**O padrão de regressão do projeto** está documentado por um caso real: o incidente
de escala monetária de 2026-07-22 gerou (1) correção em três camadas, (2) migração de
saneamento, (3) um teste nomeado pelo incidente —
`legacy_high_scale_money_still_renders_the_wallet` — que planta o estado anterior e
confirma que a leitura funciona.

Esse é o modelo esperado: **todo defeito de dinheiro vira teste nomeado**.

## 8. Cobertura esperada

Não há ferramenta de cobertura configurada, então **não existe percentual medido**.
Ver [coverage.md](coverage.md) para a proposta.

A expectativa qualitativa, por módulo:

| Módulo | Expectativa | Estado |
| --- | --- | --- |
| `repository` (dinheiro) | Todo caminho de escrita e toda guarda | 26 testes — atendido |
| `auth/*` | Todo mecanismo de defesa | 8 testes — atendido |
| `services/portfolio` | Montagem e propagação de erro | 7 testes — atendido |
| `routes/*` | Renderização, autorização, contrato | 22 + 23 testes — atendido |
| `market`, `quotes` | Parse e projeção | 13 + 12 testes — atendido |
| `app.rs` | Boot, camadas, sondas | **Parcial** — as camadas são exercitadas indiretamente; `init_otel` e `shutdown_signal` não têm teste |
| `config.rs` | Validação de ambiente | **Nenhum teste direto** |
| `i18n` | Resolução de idioma | 4 testes — atendido |

> **`config.rs` sem teste é a lacuna mais concreta.** A validação *fail-fast* é o que
> impede o serviço de subir sem `JWT_SECRET`, e nada verifica que ela funciona.
> Registrado em [../decisions/technical-debt.md](../decisions/technical-debt.md).

## 9. Riscos da atividade de teste

| # | Risco | Impacto | Mitigação atual |
| --- | --- | --- | --- |
| R1 | Suíte verde com JS quebrado | Operação inerte para quem tem JS | Caminho sem JS permanece funcional |
| R2 | Migração `.down.sql` falha quando necessária | Rollback impossível | **Nenhuma** |
| R3 | Query nova de agregado sem `ROUND` | Repetição do incidente de 500 | Teste de regressão pega o estado antigo, **não** uma query nova |
| R4 | Payload externo muda sem recaptura | Falso verde | Testes de contrato pegam o **formato**, não o conteúdo do dia |
| R5 | Cobertura desconhecida | Lacuna invisível | **Nenhuma** |
| R6 | Sem execução agendada | Advisory novo passa despercebido | **Nenhuma** |
| R7 | Autor único revisando o próprio código | Ponto cego | Testes verificam comportamento, não intenção |

## 10. Responsabilidades

Projeto de autor único: todos os papéis são do mesmo responsável
([`enoquesousa`](https://github.com/enoquesousa)). Declarado explicitamente para não sugerir uma estrutura que não existe.

| Papel | Responsabilidade |
| --- | --- |
| Desenvolvimento | Escrever o teste junto da mudança |
| Revisão | CI como revisor automatizado |
| Aprovação de release | Critérios da §6 |
| Manutenção da suíte | Atualizar o catálogo ao adicionar ou remover teste |

## 11. Evidências

```text
- .github/workflows/ci.yml   (os 4 jobs, gatilhos e ordem)
- tests/                     (35 testes de contrato)
- src/**/mod tests           (83 testes de unidade)
- tests/payloads/README.md   (política e recaptura)
- src/routes/snapshots/      (3 snapshots)
- tests/fixtures/, src/routes/fixtures/
```
