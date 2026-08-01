# Arquitetura do wallet-live

Este documento explica **como o sistema é montado, etapa por etapa, e por
que cada peça existe** — extraído inteiramente do código-fonte atual. Cada
seção cita o arquivo e o símbolo (função, tipo, constante ou teste) que
sustenta a afirmação — não o número da linha, que muda a cada commit sem
avisar.

## Escopo

Este documento cobre a **montagem do sistema**: camadas, fluxo de requisição,
modelo de dados, sessão, concorrência, observabilidade e build. Ele não cobre:

| Assunto | Documento |
| --- | --- |
| Ficha individual de cada componente | [component-architecture.md](component-architecture.md) |
| Diagramas de fluxo e sequência | [data-flow.md](data-flow.md) |
| Por que cada tecnologia foi escolhida | [technology-decisions.md](technology-decisions.md) |
| Decisões arquiteturais individuais, com alternativas | [../adr/](../adr/) |
| Contratos HTTP e payloads | [../api/](../api/) |
| Schema e dicionário de dados | [../data/](../data/) |
| Modelo de ameaças e riscos residuais | [../security/threat-model.md](../security/threat-model.md) |
| Catálogo de testes | [../testing/test-catalogue.md](../testing/test-catalogue.md) |

## Índice

1. [Visão geral](#1-visão-geral)
2. [Anatomia de uma requisição](#2-anatomia-de-uma-requisição)
3. [As camadas: routes → services → repository → banco](#3-as-camadas-routes--services--repository--banco)
4. [O modelo de dados: a evolução contada pelas migrations](#4-o-modelo-de-dados-a-evolução-contada-pelas-migrations)
5. [Dinheiro exato: `Decimal`, `MONEY_SCALE` e o incidente que provou por quê](#5-dinheiro-exato-decimal-money_scale-e-o-incidente-que-provou-por-quê)
6. [Autenticação: JWT curto + refresh token com rotação](#6-autenticação-jwt-curto--refresh-token-com-rotação)
7. [Autorização: `Admin` com dois caminhos](#7-autorização-admin-com-dois-caminhos)
8. [CSRF, lockout de login e cabeçalhos de segurança](#8-csrf-lockout-de-login-e-cabeçalhos-de-segurança)
9. [Erro central: `AppError` e a censura de 5xx](#9-erro-central-apperror-e-a-censura-de-5xx)
10. [Frontend: Askama + htmx + i18n](#10-frontend-askama--htmx--i18n)
11. [Mercado e cotações: dois jobs, duas primitivas de concorrência diferentes](#11-mercado-e-cotações-dois-jobs-duas-primitivas-de-concorrência-diferentes)
12. [Observabilidade: tracing, OpenTelemetry, sondas](#12-observabilidade-tracing-opentelemetry-sondas)
13. [Testes: a pirâmide real do projeto](#13-testes-a-pirâmide-real-do-projeto)
14. [Build e deploy](#14-build-e-deploy)

---

## 1. Visão geral

O `wallet-live` é **um único binário Rust** (Axum + SQLx/Postgres + Askama)
que serve tanto uma API REST administrativa (`/api`, `/api/v1`) quanto uma
interface web renderizada no servidor (SSR) para o usuário final — sem
frontend separado, sem build de JavaScript, sem microsserviço adicional.
Templates e migrações são **compilados dentro do binário**
(`#[derive(Template)]` do Askama, `sqlx::migrate!()`); o único artefato de
deploy é esse binário mais uma imagem base mínima (ver
[§14](#14-build-e-deploy)).

Três princípios aparecem repetidos em quase toda decisão de código deste
projeto, e vale nomeá-los antes de entrar em cada peça:

- **Dinheiro nunca é ponto flutuante.** Todo valor monetário é
  `rust_decimal::Decimal` ↔ `NUMERIC` do Postgres, ponta a ponta — sem
  exceção. A seção 5 mostra o incidente real que motivou reforçar isso.
- **Falha rápido, na borda.** Configuração ausente derruba o boot
  ([src/config.rs](../../src/config.rs)); entrada inválida é rejeitada no
  repository, antes de qualquer escrita ([src/repository.rs](../../src/repository.rs));
  erro de negócio nunca é confundido com erro interno
  ([src/error.rs](../../src/error.rs)).
- **Progressive enhancement, não JavaScript por padrão.** Toda tela
  funciona com formulário HTML puro; htmx e o único script próprio
  (`money-input.js`) são camadas que melhoram a experiência quando
  disponíveis, nunca um requisito (seção 10).

## 2. Anatomia de uma requisição

O `Router` é montado em [src/app.rs](../../src/app.rs) (`App::start`) como uma
pilha de camadas (`layer`), aplicadas de dentro para fora — o Axum executa a
**última** `.layer()` adicionada **primeiro**. Na ordem em que a requisição
de fato passa:

```
request_tracing        (mais externa: abre o span, mede latência, injeta x-request-id)
  └─ security_headers   (CSP, X-Frame-Options, HSTS, no-store...)
       └─ refresh_session  (renova a sessão ANTES de qualquer handler rodar)
            └─ Router     (rotas de /api, /api/v1 e do frontend, cada uma com seus próprios extratores)
```

O porquê da ordem, direto dos comentários do código
([src/app.rs · App::router](../../src/app.rs)):

- `request_tracing` é a camada **mais externa** de propósito: assim até os
  logs dos middlewares internos saem correlacionados ao mesmo
  `request_id`, e a métrica de duração cobre a requisição inteira,
  cabeçalhos de segurança inclusos.
- `security_headers` vem antes do roteamento porque se aplica a **toda**
  resposta, inclusive erros e 404 — não é algo que cada handler precisa
  lembrar de fazer.
- `refresh_session` roda antes de qualquer handler para que, se o access
  token expirou mas o refresh ainda é válido, a sessão já esteja
  renovada (usuário reconstruído nas `extensions` da requisição) **antes**
  do extrator `User` de cada rota tentar ler o cookie — sem essa ordem, o
  handler veria uma sessão expirada mesmo com um refresh válido em mãos.

Dentro do `Router`, cada handler declara como parâmetros os extratores de
que precisa (`State<AppState>`, `User`, `Repository`, `PortfolioService`,
`Locale`...) — o Axum resolve cada um chamando
`FromRequestParts::from_request_parts` antes de o corpo do handler rodar.
Se um extrator falha (ex.: `User` sem cookie válido), o handler **nunca
executa** — a proteção de uma rota é visível na sua assinatura, não numa
lista de exceções em outro lugar (contraste explícito com o middleware
global de autenticação típico do Rocket, registrado em
[ADR-0002](../adr/0002-axum-em-vez-de-rocket.md)).

## 3. As camadas: routes → services → repository → banco

```
src/routes/{api,frontend}.rs   — só HTTP: parsing de formulário/JSON, CSRF,
                                  redirect, flash message, escolha entre
                                  fragmento htmx e página inteira
src/services/portfolio.rs      — orquestra consultas concorrentes e regras
                                  de composição de tela (paginação, gráfico)
src/repository.rs              — TODO o SQL do sistema; validação de entrada
                                  na borda da escrita
src/models.rs                  — os tipos de dado que atravessam as camadas
```

A regra que mantém essa separação real (não só nominal) é visível em
`PortfolioService` ([src/services/portfolio.rs](../../src/services/portfolio.rs)):
ele é genérico sobre um trait, `PortfolioRepository`, não sobre o
`Repository` concreto —

```rust
pub struct PortfolioService<R: PortfolioRepository = Repository> {
    repository: R,
}
```

— o que permite testar a **montagem** da visão da carteira (que consulta
chamar, como paginar, o que fazer com o resultado de uma operação) contra um
`FakeRepository` em memória, sem Postgres, e testar a **matemática
financeira** (custo médio, saldo, guardas de concorrência) separadamente,
contra Postgres real, em `repository.rs`. Nenhum teste testa as duas coisas
ao mesmo tempo.

Os handlers em `routes/frontend.rs` são deliberadamente burros: `render_wallet`
([src/routes/frontend.rs · render_wallet](../../src/routes/frontend.rs)) só garante o token
CSRF, pede a visão pronta ao serviço, e decide entre devolver a página
inteira (`AssetsPage`) ou o fragmento (`WalletFragment`) conforme o header
`hx-request` — nenhuma regra de negócio mora ali.

## 4. O modelo de dados: a evolução contada pelas migrations

As 11 migrações em [migrations/](../../migrations/) não são um schema
desenhado de uma vez — são uma sequência real de decisões, cada uma
resolvendo um problema que a anterior não previa. Na ordem:

| Data | Migração | O que resolveu |
| --- | --- | --- |
| 2026-06-02 | `create_assets` | Catálogo de ativos: `id`, `name` único, `unit_value` — inicialmente `DOUBLE PRECISION`. |
| 2026-06-03 | `create_users` | `username` único, `password_hash` — nunca senha em texto livre. |
| 2026-06-04 | `create_owned_assets` | Histórico de compras por usuário (append-only). |
| 2026-06-13 | `money_to_numeric` | **`unit_value` sai de `DOUBLE PRECISION` para `NUMERIC`** — o comentário da própria migração nomeia o motivo: "`DOUBLE PRECISION` carrega ruído de arredondamento (0,1 + 0,2 ≠ 0,3) inaceitável para valor financeiro". |
| 2026-06-13 | `add_user_balance` | Saldo em caixa por usuário. |
| 2026-06-13 | `holdings_and_transactions` | **Reformulação central do domínio** — ver abaixo. |
| 2026-07-16 | `financial_guardrails` | `CHECK` de preço/quantidade não-negativos no schema — "última linha de defesa". |
| 2026-07-16 | `create_sessions` | Tabela de sessões para o refresh token (seção 6). |
| 2026-07-17 | `user_roles` | Coluna `role` (`user`/`admin`) — autorização passa a poder derivar de sessão, não só do secret. |
| 2026-07-18 | `portfolio_snapshots` | Série temporal de patrimônio, para o gráfico de evolução. |
| 2026-07-22 | `normalize_money_scales` | **Correção de um incidente de produção real** — ver seção 5. |

### A reformulação de `owned_assets` em `holdings` + `transactions`

O comentário no topo de
[20260613000002_holdings_and_transactions.up.sql](../../migrations/20260613000002_holdings_and_transactions.up.sql)
é a explicação mais direta de uma decisão de arquitetura em todo o
repositório:

> "O curso modelou `owned_assets` como um log de compras *append-only* e
> derivava tudo (quantidade possuída, lucro/prejuízo) agregando isso em
> tempo de leitura. Isso funciona para só-compra, mas uma carteira de
> verdade também vende, então separamos a preocupação em duas: `holdings`
> (a posição atual por usuário/ativo — quanto se possui e o custo médio,
> mutada atomicamente na compra/venda) e `transactions` (o livro-razão
> imutável de tudo que aconteceu, para o histórico e auditoria)."

Na prática:
- `holdings` tem chave primária composta `(user_id, asset_id)` — uma linha
  por posição, sempre `quantity >= 0` (a linha é **apagada**, não zerada,
  quando a posição fecha — ver `sell_asset` em
  [src/repository.rs · sell_asset](../../src/repository.rs)).
- `transactions` é só-inserção, com `kind` restrito por `CHECK` a
  `'deposit'/'buy'/'sell'` e `cash_delta` assinado (depósito positivo, compra
  negativa, venda positiva) — é a fonte de verdade do extrato e do CSV
  exportável.
- A migração faz a transição dos dados existentes com um `INSERT...SELECT`
  agregando `owned_assets` para popular `holdings`, e outro para
  reconstituir `transactions` a partir do mesmo histórico — nenhum dado é
  perdido na transição de modelo.

O ganho prático: `wallet_summary` e `list_holdings`
([src/repository.rs · wallet_summary/list_holdings](../../src/repository.rs)) são consultas triviais
(um `JOIN`, sem agregação pesada), porque a agregação já aconteceu no
momento da escrita (`buy_asset`/`sell_asset`), não a cada leitura.

### Guardas em camadas, não só na aplicação

`financial_guardrails` adiciona `CHECK (unit_value >= 0)` em `assets` e
`CHECK (quantity IS NULL OR quantity > 0)` em `transactions` — mesmo essas
condições já sendo validadas em Rust
(`validated_unit_value`/`validated_asset_name` em
[src/repository.rs · validated_asset_name/validated_unit_value](../../src/repository.rs)). O comentário da migração
é explícito: "a aplicação já valida isso na borda HTTP; o banco é a última
linha de defesa: nenhum caminho de escrita — API do admin, sincronização de
cotação, SQL manual — consegue persistir um valor inválido". É defesa em
profundidade: dois lugares diferentes concordando que "preço negativo"
nunca é um estado válido, um deles impossível de contornar mesmo por um bug
na camada Rust.

## 5. Dinheiro exato: `Decimal`, `MONEY_SCALE` e o incidente que provou por quê

`MONEY_SCALE = 8` ([src/models.rs · MONEY_SCALE](../../src/models.rs)) é o invariante mais
citado no código-fonte do projeto — e o comentário que o define já avisa do
risco:

> "`NUMERIC` do Postgres é ilimitado, mas `rust_decimal::Decimal` tem 28
> dígitos significativos: valores de escala alta (ex.: preço = 1/taxa com 28
> casas) tornam PRODUTOS e SOMAS no SQL indecodificáveis na leitura
> (`value not representable`) — derrubando a tela da carteira."

Isso não é um risco teórico: **aconteceu**. A sincronização de cotações
(`brl_price` em [src/quotes.rs · brl_price](../../src/quotes.rs)) grava `preço =
1/taxa` — uma divisão de `Decimal` que, sem arredondar, preenche a mantissa
inteira (uma dízima como 1/3 vira uma dízima de 28 casas). Um preço
individual com 28 casas ainda cabe num `Decimal`; mas **o produto ou a
soma** desse valor com outro (exatamente o que `wallet_summary`,
`list_holdings` e `record_portfolio_snapshots` fazem) estoura o limite de 28
dígitos significativos, e a leitura de volta falha.

A correção tem três camadas, todas visíveis no código:

1. **Escrita passou a arredondar sempre.** `brl_price` arredonda para
   `MONEY_SCALE` antes de gravar
   ([src/quotes.rs · brl_price](../../src/quotes.rs)); `validated_unit_value` faz o
   mesmo para qualquer escrita administrativa
   ([src/repository.rs · validated_unit_value](../../src/repository.rs)); `buy_asset`/`sell_asset`
   arredondam o produto preço×quantidade antes de usá-lo
   (`round_dp(MONEY_SCALE)`, [src/repository.rs · buy_asset](../../src/repository.rs)).
2. **Leitura passou a `ROUND(...)` defensivamente.** Todo agregado SQL que
   soma ou multiplica `NUMERIC` (`wallet_summary`, `list_holdings`,
   `record_portfolio_snapshots`) envolve o resultado em `ROUND(..., 8)` —
   um comentário no repository nomeia isso: "produtos e somas de `NUMERIC`
   acumulam escala sem limite... a leitura falharia mesmo com cada coluna
   dentro do invariante" ([src/repository.rs · wallet_summary](../../src/repository.rs)).
3. **A migração `normalize_money_scales` saneou o estado já gravado** —
   arredondando para 8 casas qualquer `unit_value`/`avg_cost`/`balance`/
   `total_value` que já estivesse acima disso no banco em produção.

E existe um teste de regressão nomeado pelo incidente:
`legacy_high_scale_money_still_renders_the_wallet`
([src/repository.rs · legacy_high_scale_money_still_renders_the_wallet](../../src/repository.rs)) planta deliberadamente
valores de 28 casas no banco (simulando o estado anterior à correção) e
confirma que toda leitura (`wallet_summary`, `list_holdings`, snapshot,
nova compra) continua decodificando corretamente graças ao `ROUND` — o teste
existe especificamente para que essa classe de bug não volte.

## 6. Autenticação: JWT curto + refresh token com rotação

Dois tokens, dois cookies, dois propósitos:

- **Access token** (cookie `token`, [src/auth/user.rs · TOKEN_COOKIE](../../src/auth/user.rs)) —
  um JWT HS256 assinado (`jwt_simple`), com claims customizadas (`id`,
  `username`, `role`), válido por `SESSION_TTL_MINUTES` (10 min por
  padrão). **Stateless**: validá-lo não toca o banco, só confere a
  assinatura com o `JWT_SECRET`.
- **Refresh token** (cookie `refresh_token`,
  [src/auth/session.rs · RefreshToken](../../src/auth/session.rs)) — 32 bytes
  aleatórios do SO, **opaco** (não carrega dado nenhum, diferente do JWT).
  Só a **hash SHA-256** dele é gravada na tabela `sessions`; o valor em
  claro nunca toca o banco — "um vazamento do banco não vaza token
  utilizável" (comentário na migração `create_sessions`).

A renovação acontece no middleware `refresh_session`
([src/auth/session.rs · refresh_session](../../src/auth/session.rs)), descrito na seção 2:
se o access expirou mas o refresh ainda é válido, `rotate_session`
([src/repository.rs · rotate_session](../../src/repository.rs)) faz, **numa transação
só**, `UPDATE sessions SET revoked_at = NOW() WHERE token_hash = $1 AND
revoked_at IS NULL AND expires_at > NOW() RETURNING user_id` seguido de um
novo `INSERT`. O comentário explica por que o `UPDATE...RETURNING` é a peça
central: ele "reivindica" a sessão numa operação atômica — se um token
roubado e o legítimo tentarem rotacionar ao mesmo tempo, o segundo a chegar
encontra a sessão já revogada (`revoked_at IS NULL` não bate mais) e recebe
`None`. Não há janela de corrida em que os dois consigam rotacionar.

Logout ([src/routes/frontend.rs · logout](../../src/routes/frontend.rs)) chama
`revoke_session`, marcando `revoked_at` no banco — não é só apagar o
cookie do navegador; a sessão morre no servidor.

## 7. Autorização: `Admin` com dois caminhos

O extrator `Admin` ([src/auth/admin.rs](../../src/auth/admin.rs)) aceita
**duas** credenciais, nessa ordem:

1. Uma sessão de usuário cujo `role` é `admin` (lido das claims do JWT,
   já assinadas — nenhuma consulta extra ao banco).
2. O header `Authorization` batendo com `ADMIN_SECRET_KEY`, comparado em
   **tempo constante** (`subtle::ConstantTimeEq`) — para o tempo de resposta
   não vazar, byte a byte, quanto do segredo bateu antes de divergir.

Um detalhe deliberado no código: se existe uma sessão válida mas o usuário
**não** é admin, a função retorna erro imediatamente, sem cair para checar o
header ([src/auth/admin.rs · Admin::from_request_parts](../../src/auth/admin.rs)) — "ele claramente
está usando a sessão. Negar já." Evita o caso estranho de um usuário comum
autenticado conseguir autorização só porque, por coincidência, mandou um
header `Authorization` de outra finalidade.

## 8. CSRF, lockout de login e cabeçalhos de segurança

**CSRF** ([src/auth/csrf.rs](../../src/auth/csrf.rs)): *double-submit cookie* —
o servidor gera um token aleatório, grava num cookie **e** embute o mesmo
valor num campo oculto do formulário renderizado; no POST, os dois têm que
bater (comparação também em tempo constante). O comentário no código é
honesto sobre a camada de defesa: `SameSite=Strict` já bloqueia a maior
parte do CSRF em navegadores modernos; isto é defesa em profundidade para
navegadores antigos ou brechas de same-site.

**Lockout de login** ([src/auth/throttle.rs](../../src/auth/throttle.rs)):
`LoginThrottle` conta falhas consecutivas **por usuário** (normalizado —
`trim().to_lowercase()`, para maiúscula não escapar do bloqueio) e, a
partir de 5 tentativas, impõe backoff exponencial (30 s dobrando até um
teto de 15 min). A checagem roda **antes** de conferir a senha
(`authenticate_form` em
[src/routes/frontend.rs · authenticate_form](../../src/routes/frontend.rs)) — durante o
bloqueio, nem a senha certa passa, o que impede um ataque de força bruta
extrair qualquer sinal das tentativas.

**Cabeçalhos de segurança** (`security_headers` em
[src/app.rs · security_headers](../../src/app.rs)), aplicados a toda resposta: CSP
fechada (`script-src 'self'`, `style-src 'self'`, sem `'unsafe-inline'` —
possível porque não existe CSS/JS inline em nenhuma página, um invariante
travado por teste, ver seção 13), `X-Content-Type-Options: nosniff`,
`X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, `Cache-Control:
no-store` em toda resposta que não é asset estático (telas autenticadas não
podem ficar em cache de navegador/proxy), e HSTS condicional a
`cookie_secure` (não faz sentido em HTTP local).

## 9. Erro central: `AppError` e a censura de 5xx

Um único enum, [src/error.rs](../../src/error.rs), reúne todo erro possível do
sistema — 21 variantes, cada uma mapeada para um `StatusCode` específico em
`IntoResponse` (não um genérico 500 para tudo). A peça mais importante do
arquivo é a distinção entre erro do cliente e erro do servidor
([src/error.rs · IntoResponse for AppError](../../src/error.rs)):

```rust
let error = if status.is_server_error() {
    tracing::error!(error = ?self, "internal error serving request");
    "internal server error".to_string()
} else {
    self.to_string()
};
```

Erros 4xx (senha errada, saldo insuficiente, CSRF divergente) devolvem a
mensagem real — não revelam nada sobre como o sistema funciona por dentro.
Erros 5xx (falha de banco, template mal configurado) são logados
**inteiros**, com a causa raiz (o `#[from]`/`#[error(transparent)]` do
`thiserror` encadeia a causa automaticamente), e o cliente recebe só
`"internal server error"` — nunca o texto de erro do SQL, nome de coluna ou
string de conexão.

Duas conversões merecem nota:
- `AppError::Database(#[from] sqlx::Error)` — qualquer `?` sobre uma
  chamada SQLx vira `AppError` automaticamente.
- `AppError::Jwt(String)` — **não** usa `#[from]`/`transparent` porque
  `jwt_simple::Error` não implementa `std::error::Error` (é um
  `anyhow::Error` por baixo); a conversão é um `impl From` manual guardando
  só a mensagem como string ([src/error.rs · From<jwt_simple::Error>](../../src/error.rs)).

## 10. Frontend: Askama + htmx + i18n

**Templates.** Todo HTML nasce de `#[derive(Template)]` (Askama) —
checagem em tempo de compilação de que toda variável usada no `.html` existe
na struct correspondente. Duas structs por tela (`AssetsPage`/
`WalletFragment`, `MarketPage`/`MarketFragment`) compartilham o mesmo tipo
de dado (`WalletData`, `MarketData`) e o mesmo fragmento interno — a
diferença é só o esqueleto ao redor (ver `render_wallet` em
[src/routes/frontend.rs](../../src/routes/frontend.rs)).

**htmx sem substituir o clássico.** Toda ação (depositar, comprar, trocar
de moeda no mercado) tem **dois caminhos simultâneos** no mesmo HTML: o
`action`/`method` de um formulário POST normal, e os atributos `hx-*`
(`hx-post`, `hx-target`, `hx-swap`) que interceptam o mesmo clique via
JavaScript, quando disponível, e trocam só o fragmento correspondente. Uma
requisição htmx se anuncia pelo header `hx-request`
(`is_partial_request` em
[src/routes/frontend.rs · is_partial_request](../../src/routes/frontend.rs)); o handler
devolve o fragmento (`WalletFragment`) ou a página inteira
(`AssetsPage`) conforme esse sinal — nunca dois códigos de handler
diferentes para o mesmo dado.

**A CSP que isso protege.** Nenhum `<script>` inline existe em nenhuma
página — `pages_carry_no_inline_style_or_script`
([src/routes/frontend.rs](../../src/routes/frontend.rs), seção de testes) trava
esse invariante: itera todo `<script` de cada página renderizada e falha se
algum não tiver `src=`. htmx e a máscara monetária
(`static/money-input.js`) são arquivos externos servidos do próprio
binário via `include_str!` — nunca CDN de terceiro.

**Internacionalização** ([src/i18n.rs](../../src/i18n.rs)): cada idioma é uma
`static Strings` — uma struct com um campo por texto da interface. Um texto
faltando num idioma é **erro de compilação**, não uma chave ausente
descoberta em produção. A resolução de idioma segue a ordem cookie
explícito → `Accept-Language` do navegador → `pt-BR` como padrão
(`resolve` em [src/i18n.rs · resolve](../../src/i18n.rs)). Formatação de dinheiro,
data e CSV seguem a convenção do **dado** (BRL, pt-BR), não da interface —
mesmo a tela em inglês mostra `R$ 10,00`, não `$10.00`, porque o valor é
brasileiro independente do idioma de quem olha.

## 11. Mercado e cotações: dois jobs, duas primitivas de concorrência diferentes

Dois jobs em segundo plano, `tokio::spawn`-ados uma vez no boot
([src/app.rs · App::start](../../src/app.rs)), nunca no caminho de uma requisição:

- **`quotes::spawn_scheduled_sync`** ([src/quotes.rs](../../src/quotes.rs)) —
  atualiza os preços que **lastreiam operações reais** (compra/venda),
  buscando taxas de câmbio da Coinbase. Protegido por
  `QuoteSync { last_finished: Mutex<Option<Instant>> }`: o mutex fica
  adquirido durante a rodada inteira, então duas chamadas simultâneas
  (o botão manual e o job agendado, por exemplo) nunca disparam duas
  requisições nem gravam dois snapshots — é exclusão mútua de verdade,
  porque escrever o catálogo administrativo duas vezes ao mesmo tempo seria
  um problema real.
- **`market::spawn_scheduled_refresh`** ([src/market.rs](../../src/market.rs)) —
  atualiza o snapshot **informativo** de 100 criptomoedas (CoinGecko),
  exibido na tela de mercado. Protegido por `RwLock<Snapshot>`, não
  `Mutex`: toda requisição HTTP **lê** o snapshot, só o job **escreve**
  (uma vez por minuto) — `RwLock` permite leituras concorrentes sem fila,
  o encaixe certo quando leitura domina esmagadoramente sobre escrita.

Os dois nunca se misturam: o comentário em `AppState`
([src/app.rs · AppState::market](../../src/app.rs)) é explícito — o snapshot de mercado
"fica fora do banco de propósito: é dado de terceiro, volátil e puramente
informativo... misturaria cotação de fora com o catálogo que lastreia as
operações". `market.rs` também documenta um detalhe de integração
descoberto na prática: a CoinGecko responde **403 sem `User-Agent`**
([src/market.rs](../../src/market.rs), comentário em `USER_AGENT`) — o
`reqwest` não manda um por padrão, e sem isso o feed nunca sobe.

## 12. Observabilidade: tracing, OpenTelemetry, sondas

**`tracing` + `#[instrument]`** em praticamente todo handler — cada
requisição vira um span (`request`, com `request_id`, método e caminho),
propagado por todos os spans filhos dos handlers/middlewares que rodam
dentro. **Exportação OTLP é opt-in**: só liga se
`OTEL_EXPORTER_OTLP_ENDPOINT` estiver definida; ausente, zero tentativa de
conexão e zero overhead (`init_otel` em
[src/app.rs · init_otel](../../src/app.rs)). Junto do trace, um histograma
(`http.server.request.duration`) é alimentado a cada requisição, rotulado
por método, rota (não a URL crua — evita cardinalidade ilimitada de um 404
em path aleatório) e status.

**Duas sondas de saúde, com propósitos diferentes**
([src/app.rs · liveness/readiness](../../src/app.rs)):
- `/healthz` (liveness) — não toca o banco. Se falhar, o orquestrador deve
  **reiniciar** o processo.
- `/readyz`/`/health` (readiness) — exige `SELECT 1` no Postgres. Se
  falhar, o orquestrador tira a instância do balanceador **sem reiniciá-la**
  — reiniciar o app não conserta um Postgres fora do ar.

`request_id` é propagado do header `x-request-id` quando um proxy já o
gerou (validado contra caracteres/tamanho antes de aceitar,
[src/app.rs · request_tracing](../../src/app.rs)), senão gerado localmente (8 bytes
aleatórios em hexa) — e sempre devolvido na resposta, para correlacionar um
erro reportado pelo cliente com a linha exata de log no servidor.

## 13. Testes: a pirâmide real do projeto

Quatro níveis distintos, cada um testando uma coisa diferente:

- **Testes de repositório** (`#[sqlx::test]`,
  [src/repository.rs](../../src/repository.rs)) — banco efêmero real por
  teste (migrado automaticamente, isolado, paralelo). Cobrem a matemática
  financeira exata: custo médio ponderado, guardas de saldo/posição,
  arredondamento, e o teste de regressão do incidente de escala (seção 5).
- **Testes de orquestração** (`FakeRepository` em memória,
  [src/services/portfolio.rs](../../src/services/portfolio.rs)) — sem banco
  nenhum, testam só a montagem da `WalletView` e a propagação de erro. São
  os mais rápidos e não competem por conexão de banco no CI.
- **Testes de renderização** (`AssetsPage::render()`, `MarketFragment::render()`,
  [src/routes/frontend.rs](../../src/routes/frontend.rs)) — verificam
  invariantes de HTML puro: nenhum `<style>`/`<script>` inline, variação
  sempre com seta **e** sinal (não só cor — acessibilidade para
  deuteranopia, medida em ΔE), os dois idiomas renderizando.
- **Snapshots de contrato de API** (`insta::assert_json_snapshot!`,
  [src/routes/api.rs](../../src/routes/api.rs)) — travam o formato JSON exato
  de resposta; uma mudança de contrato exige `cargo insta review`
  explícito, nunca passa despercebida.

O CI ([.github/workflows/ci.yml](../../.github/workflows/ci.yml)) roda três
frentes independentes: `lint` (fmt + clippy com `-D warnings`, mais um
check de que `static/app.css` compilado bate com `styles/app.css` — sem
precisar de banco, graças ao cache `SQLX_OFFLINE`), `test` (suíte completa
contra Postgres real em service container, mais `cargo sqlx prepare
--check` garantindo que o cache offline não descolou do schema), e `audit`
(dependências com vulnerabilidade conhecida via RustSec).

## 14. Build e deploy

**Build multi-stage** ([Dockerfile](../../Dockerfile)): o estágio `builder`
(`rust:1.95-slim`) compila com `SQLX_OFFLINE=true` — o binário nasce sem
nunca ter falado com um banco, usando só o cache `.sqlx/` versionado no
repositório. O estágio `runtime` (`debian:bookworm-slim`) copia **só o
binário** de `target/release/wallet`; nenhuma toolchain de build, nenhum
código-fonte, nenhuma dependência de compilação vai para a imagem final. O
processo roda como usuário sem privilégio (`useradd --system --uid 10001
wallet`) — um comprometimento não ganha root.

**Migrações no boot** ([src/app.rs · AppState::build](../../src/app.rs), dentro de
`AppState::build`): `sqlx::migrate!().run(&db).await?` roda antes do
serviço aceitar qualquer requisição, e é idempotente (migração já aplicada é
pulada). Falhar aqui derruba o boot — "melhor não subir do que subir contra
um schema pela metade".

**`docker-compose.yaml`** separa três perfis:
- `db` (padrão, sempre sobe) — Postgres para desenvolvimento local com
  `cargo run` direto.
- `app` (perfil `app`, opcional) — builda e sobe a imagem de produção
  completa, healthcheck batendo em `/readyz`.
- `otel-collector` (perfil `observability`, opcional) — coletor OTLP local
  só para inspecionar o que o serviço exportaria, sem montar um backend de
  observabilidade de verdade.

Essa separação existe porque o ciclo do dia a dia (editar código, `cargo
run`) e o ciclo de validar o artefato de produção (imagem Docker completa)
têm necessidades diferentes — o primeiro não deveria pagar o custo de
rebuildar a imagem a cada mudança de uma linha.
