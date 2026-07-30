# Estratégia de testes

## Objetivo

Explicar **como** o projeto testa, **por que em dois níveis** e **o que
deliberadamente não é testado**. O inventário caso a caso — o que cada teste
trava e por que existe — fica em [test-catalogue.md](test-catalogue.md); a
matriz de risco em [test-matrix.md](test-matrix.md); a política de execução em
[test-plan.md](test-plan.md).

## Escopo

Coberto: níveis de teste, escolha de dublês, política de payloads externos,
execução, e os limites conhecidos da suíte. Não coberto: cobertura instrumentada
(ver [coverage.md](coverage.md), que registra a **ausência** de ferramenta) e
formato de relatório (ver [test-report-template.md](test-report-template.md)).

---

## 1. Números atuais

**118 testes**, distribuídos assim (contagem por atributo `#[test]`,
`#[tokio::test]` e `#[sqlx::test]`):

| Camada | Testes | Onde |
| --- | ---: | --- |
| Unidade | 83 | `src/**` em `#[cfg(test)] mod tests` |
| Contrato / integração | 35 | `tests/*.rs` |

Distribuição por módulo: `repository` 26, `routes::frontend` 18, `http_web` 15,
`market` 11, `http_api` 8, `payload_market` 7, `services::portfolio` 7,
`payload_quotes` 5, `i18n` 4, `routes::api` 4, `auth::throttle` 4,
`auth::csrf` 3, `routes::flash` 3, `quotes` 2, `auth::user` 1.

> Esta é a única contagem mantida no repositório. Outros documentos referenciam
> este número em vez de repeti-lo, para não haver três verdades divergentes
> depois do próximo teste escrito.

## 2. As duas camadas, e por que são duas

**Unidade — `src/**/#[cfg(test)] mod tests` (83).** Moram ao lado do código que
testam e têm acesso ao que é privado: a projeção de um gráfico no `viewBox`, a
inversão de uma taxa de câmbio, a montagem de uma URL, o cálculo de custo médio
contra um Postgres efêmero. É o idioma de Rust para internos, e é onde a
asserção pode ser precisa — comparar a string exata de um `<path>` SVG, por
exemplo.

**Contrato — `tests/*.rs` (35).** São crates separados: só alcançam o que é
público, e é essa restrição que os torna úteis. Eles atravessam as **mesmas
funções que o servidor atravessa**, com as **mesmas entradas que a rede
entrega** — um payload JSON real de terceiro, uma requisição HTTP passando por
todos os middlewares. É a camada que responde "isto funciona montado?", que
nenhum teste de unidade pode responder.

A pasta `tests/` só existe porque o crate ganhou um alvo de biblioteca
(`src/lib.rs`): testes de integração não conseguem importar de um binário.
Enquanto tudo vivia em `main.rs`, a única camada possível era a de unidade.

Evidência:
- `src/lib.rs` — o alvo de biblioteca que habilita a camada de contrato
- `tests/common/mod.rs` — o andaime que monta o app de produção sem abrir socket

## 3. Classificação por tipo

O pedido de documentação lista 22 tipos possíveis de teste. Esta é a situação
real de cada um — inclusive os ausentes, que estão marcados como tal em vez de
omitidos:

| Tipo | Situação | Onde / por que não |
| --- | --- | --- |
| Unitário | Coberto | 83 testes, ver catálogo |
| Integração (com banco real) | Coberto | `#[sqlx::test]` em `repository`, `routes::api` |
| Contrato (HTTP) | Coberto | `tests/http_api.rs`, `tests/http_web.rs` |
| Contrato (payload de terceiro) | Coberto | `tests/payload_quotes.rs`, `tests/payload_market.rs` |
| Componentes (renderização) | Coberto | `routes::frontend` — HTML comparado como texto |
| Snapshot de contrato JSON | Coberto | `insta` em `routes::api` |
| Regressão | Coberto | `legacy_high_scale_money_still_renders_the_wallet` |
| Autenticação | Coberto | `http_web` (gate de sessão, expiração, htmx) |
| Autorização | Coberto | `http_api::writing_to_the_catalogue_requires_the_admin_credential` |
| Segurança (cabeçalhos) | Coberto | `http_api::every_api_response_carries_the_security_headers` |
| Segurança (CSRF) | Coberto | `auth::csrf` (3) + `http_web` (forjado/ausente/vazio) |
| Segurança (força bruta) | Coberto | `auth::throttle` (4) |
| Segurança (open redirect) | Coberto | `language_switch_only_follows_local_absolute_paths` |
| End-to-end (caminho do usuário) | Parcial | `the_money_flow_works_end_to_end_through_the_browser_path` percorre depósito→compra→venda pelo router real, mas **sem navegador**: nenhum JavaScript executa |
| Interface / usabilidade técnica | Parcial | Presença de atributos e classes é verificada; resultado visual não |
| Concorrência | **Ausente** | O `RwLock` do snapshot e o `Mutex` do cooldown não têm teste de corrida |
| Carga / estresse / performance | **Ausente** | Nenhuma medição de latência ou throughput |
| Compatibilidade (navegador) | **Ausente** | Nada executa em navegador real |
| Instalação | **Ausente** | O `docker build` do CI prova que a imagem compila, não que ela sobe e serve |
| Atualização / rollback | **Ausente** | As migrações `.down.sql` existem e são reversíveis por construção, mas **nenhum teste as executa** |
| Recuperação / resiliência | **Ausente** | Perda prolongada de conectividade e corrupção de banco não são exercitadas |
| Manual | Informal | O roteiro de demonstração em [../delivery/course-delivery.md](../delivery/course-delivery.md) é a única sequência manual registrada |

## 4. Por que payload real, e não fixture escrito à mão

Um fixture inventado testa a **nossa ideia** do formato. O formato é do outro
lado. As duas coisas divergem exatamente onde dói — dois achados concretos da
captura versionada em [`tests/payloads/`](../../tests/payloads/README.md):

- A Coinbase entrega cada taxa como **string**, com precisão arbitrária: a maior
  da captura tem **41 dígitos significativos**, mais que os 28 da mantissa do
  `Decimal`. E o mapa das 636 moedas é decodificado de uma vez — uma taxa que não
  caiba derruba a sincronização de **todos** os pares. Um fixture com
  `"BTC": 0.0000031` passaria para sempre sem nunca tocar nisso.
- A CoinGecko manda `null` em campos que o tipo declara como número, e um `roi`
  que às vezes é objeto e às vezes `null`. São 30 campos por moeda, dos quais
  lemos 15.

Nenhuma asserção depende da cotação do dia. Elas conferem invariantes (escala
travada, campo obrigatório presente, ausente virando neutro), nunca "BTC vale
R$ 327.777" — um teste que precisasse de recaptura semanal seria um alarme falso
semanal.

## 5. Escolha de dublês, e por que cada um é apropriado

| Dublê | Onde | O que substitui | Por que é apropriado aqui |
| --- | --- | --- | --- |
| `FakeRepository` (em memória) | `src/services/portfolio.rs` | Todo o acesso a banco | O alvo é a **montagem** da `WalletView` — qual consulta chamar, como paginar, o que fazer com o resultado. A aritmética financeira tem cobertura própria contra Postgres real; testar as duas coisas juntas tornaria a falha ambígua |
| Banco efêmero real (`#[sqlx::test]`) | `repository`, `routes::api` | Nada — é Postgres de verdade | As garantias que importam neste nível são **do banco**: transação, `FOR UPDATE`, `CHECK`, `UNIQUE`, `NUMERIC`. Um dublê de banco não tem nenhuma delas, e é justamente isso que o teste precisa provar |
| Payload versionado em arquivo | `tests/payload_*.rs` | Só a chamada de rede | Separar `fetch` (rede) de `parse` (decodificação) permite exercitar a decodificação — onde estão os riscos reais — sem que a suíte passe a medir a internet |
| `tower::oneshot` sobre `App::router` | `tests/http_*.rs` | O socket TCP | O router é o **de produção**, com todos os middlewares na ordem real. Só a camada de transporte é dispensada, e ela não é o que o teste investiga |
| `Admin` construído à mão | `routes::api` (unidade) | A autorização | Deliberado, e é uma **lacuna reconhecida**: nesses testes a autorização não é exercida. `tests/http_api.rs` existe para cobrir exatamente esse vão |

`#[sqlx::test]` cria um **banco efêmero por teste**, com as migrações aplicadas
automaticamente, então os testes são isolados e paralelos — nenhum depende da
ordem de execução nem do estado deixado por outro.

## 6. Como rodar

A suíte inteira precisa de um Postgres de pé:

```bash
docker compose up -d db
```

```bash
cargo test
```

Só o que não toca banco (rápido, roda em qualquer máquina):

```bash
cargo test --test payload_market --test payload_quotes
```

Uma bateria só:

```bash
cargo test --test http_web
```

Um teste só, por nome:

```bash
cargo test buying_more_averages_the_cost_basis
```

Auditar mudanças de contrato JSON depois de alterar a API:

```bash
cargo insta review
```

O CI roda `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`
e `cargo test`, nesta ordem — ver [test-plan.md](test-plan.md) para o que roda
em cada gatilho.

## 7. Como interpretar uma falha

| Sintoma | Leitura provável |
| --- | --- |
| Falha só em `tests/payload_*.rs` | A fonte externa mudou o formato (campo renomeado, tipo trocado, escala estourada). O produto ainda funciona com o snapshot atual; a próxima rodada de sincronização é que vai falhar |
| Falha em `repository` com `value not representable` | Regressão da classe do incidente de 2026-07-22: algum caminho de escrita voltou a gravar escala acima de `MONEY_SCALE`, ou um agregado SQL novo ficou sem `ROUND` |
| Falha em `routes::frontend` sobre `<script>`/`<style>` | Alguém adicionou CSS ou JS inline a um template. Passaria em revisão e o navegador o ignoraria em produção por causa da CSP |
| Falha de snapshot `insta` | O contrato JSON da API mudou. Se foi intencional, `cargo insta review`; se não, é quebra de compatibilidade para consumidores |
| Falha em `http_api`/`http_web` mas os unitários passam | O problema está na **montagem**: ordem de middleware, extrator, cookie, redirecionamento |
| Erro de conexão em todos os testes de banco | Postgres não está de pé (`docker compose up -d db`) |
| `cargo clippy` falha mas `cargo test` passa | Warning tratado como erro no CI; não é falha funcional |

## 8. O que a suíte não cobre

Honestidade sobre os limites, que é parte do desenho — e cada item tem entrada
correspondente em [../decisions/known-limitations.md](../decisions/known-limitations.md):

- **Navegador de verdade.** Nada aqui executa JavaScript. O htmx e a máscara
  monetária são verificados pelo HTML que o servidor emite (atributos `hx-*`,
  ordem dos `<script>`, `defer`), não pelo comportamento no DOM. Um erro de
  runtime no htmx passaria.
- **Layout.** Os testes conferem que as classes renderizadas existem no CSS
  compilado, não que o resultado visual está certo. O painel de mercado empilhado
  abaixo de 1024 px é comportamento correto do breakpoint — e nenhum teste
  distingue "correto" de "indesejado" aqui.
- **As chamadas HTTP externas.** `fetch` e `fetch_brl_rates` (rede) não são
  testados; o parse que eles alimentam é. Foi uma escolha: teste que bate em API
  de terceiro falha quando a rede oscila, e passa a medir a internet em vez do
  código.
- **Concorrência dos jobs.** O `RwLock` do snapshot e o `Mutex` do cooldown de
  cotações não têm teste de corrida.
- **Reversão de migrações.** Os arquivos `.down.sql` existem para as 11
  migrações, mas nenhum teste os executa — a reversibilidade é afirmada por
  construção, não verificada.
- **Carga.** Nenhuma medição de latência ou throughput.
- **Cobertura instrumentada.** Não há ferramenta configurada; o percentual real
  é desconhecido. Ver [coverage.md](coverage.md).

## 9. Evidências

```text
- src/lib.rs                          (alvo de biblioteca que habilita tests/)
- tests/common/mod.rs                 (monta App::router sem socket)
- tests/payloads/README.md            (política de payload real)
- src/services/portfolio.rs           (FakeRepository)
- .github/workflows/ci.yml            (jobs lint, test, audit, docker)
- .cargo/config.toml                  (SQLX_OFFLINE para compilar sem banco)
```

## 10. Pontos de evolução

Melhorias possíveis, **não implementadas** — listadas aqui para não serem lidas
como capacidade atual:

1. Instrumentar cobertura com `cargo-llvm-cov` e medir os módulos financeiros.
2. Um teste que aplique e reverta todas as migrações em sequência.
3. Um teste de corrida sobre `QuoteSync::run` com duas chamadas simultâneas.
4. Teste de navegador headless para o comportamento de htmx no DOM.
