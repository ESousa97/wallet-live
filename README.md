# wallet :: restful stack

Carteira digital de investimentos construída **inteiramente em Rust** — backend
e frontend. Projeto do curso *RESTful Stack* (DIO), com o instrutor Breno Lemos.

> Status: **Final — Carteira completa** ✅
> Além da API do admin e da sessão stateless via JWT, o usuário tem uma **tela de
> ativos** (`/assets`) com **saldo em caixa**, **posições** (quantidade, preço,
> custo médio e lucro/prejuízo por ativo), **resumo do patrimônio** e o
> **histórico de transações**. As operações são **depositar**, **comprar**,
> **vender** e **atualizar cotações** (preços de mercado reais via API da
> Coinbase) — tudo com valores monetários exatos (`rust_decimal`) — e **logout**.

## Estrutura

Crate: **`wallet`** (nome curto, sem traço). A UI segue a estética da demo do
curso: textos em inglês minúsculo e fonte monoespaçada.

```
src/
  main.rs            # enxuta: tokio::main -> App::start()
  app.rs             # App (boot, /health, shutdown gracioso) + AppState { db, config }
  config.rs          # Config: lê e valida os segredos do ambiente UMA vez (fail-fast)
  models.rs          # Asset, UserRecord, WalletSummary, Holding, Transaction
  error.rs           # AppError + IntoResponse (status HTTP, log de 5xx) + conversões
  quotes.rs          # cotações de mercado (Coinbase) -> atualiza preços dos ativos
  repository.rs      # Repository: todo o acesso ao banco (queries) + testes do core
  auth/
    admin.rs         # extrator Admin (autenticação por secret key, p/ a API)
    user.rs          # User/UnauthenticatedUser, hash de senha, JWT e extratores
  routes/
    api.rs           # API REST do admin (JSON) + testes (#[sqlx::test] + insta)
    frontend.rs      # front-end SSR (HTML): login/logout, carteira, operações, filtros
    fixtures/        # dados SQL para testes (bitcoin_asset.sql)
    snapshots/       # snapshots aceitos do insta (.snap)
templates/
  base.html          # esqueleto comum (head, Tailwind, fonte mono); as páginas o estendem
  login.html         # tela de login ({% extends "base.html" %})
  assets.html        # carteira: saldo, posições, resumo, transações e operações
migrations/          # assets + users + holdings/transactions + ajustes ({up,down}.sql)
docker-compose.yaml
.env / .env.example
```

## O que tem hoje

O servidor Axum serve **duas coisas** ao mesmo tempo:

1. **API REST do admin** (JSON, sob `/api`) — cadastra/lista/atualiza ativos.
2. **Front-end SSR** (HTML, na raiz) — telas de login/cadastro e index do usuário.

### Banco e repository (Aula 3)

- **Banco no estado** — `AppState { db: PgPool }`. A `PgPool` é um `Arc` por
  dentro, então clonar o estado clona só o ponteiro, não as conexões.
- **Padrão repository** — `Repository` concentra todas as queries (de ativos e de
  usuários). Os handlers não sabem como o banco funciona, só que ele existe;
  mudou o esquema, muda só o repository. Ele também é um **extrator** do Axum
  (`FromRequestParts`, `Rejection = Infallible`), injetado direto nos endpoints.
- **Queries checadas em compilação** — `sqlx::query_as!` valida cada SQL contra o
  banco real **na hora de compilar**. Se a tabela/coluna não existir, o programa
  não compila. (Por isso o Postgres precisa estar no ar para buildar.)
- **Migrações** — versionadas em `migrations/`, com `up`/`down` reversíveis
  (`create_assets` e `create_users`).
- **Testes** — `#[sqlx::test]` cria um banco efêmero por teste, roda as migrações
  e aplica *fixtures*; o **insta** garante que o JSON de resposta não mude sem
  querer (snapshot testing).

### Usuários e senha (Aula 4)

- **Modelo `users`** — `id` (BIGSERIAL), `username` (TEXT **UNIQUE** — é a chave
  de login, como um e-mail seria) e `password_hash` (TEXT).
- **Nunca senha em texto livre** — usamos a lib **`password-auth`** (argon2id por
  padrão) para gerar/verificar a hash. O `UserRecord` (linha crua do banco) **não**
  deriva `Serialize` de propósito: nada de vazar a hash numa resposta.
- **Tipos que modelam o fluxo** — `UnauthenticatedUser` (nome + senha em texto
  livre, vindos do formulário) vira um `User` (autenticado, campos privados) por
  um de dois caminhos: `authenticate` (confere a senha de um usuário existente) ou
  `register` (cadastra um novo). A única forma de obter um `User` é passando por
  um desses fluxos — tê-lo em mãos é prova de que a autenticação aconteceu.
- **Uma tela só** — para simplificar, a tela de login serve aos dois propósitos:
  se o usuário não existe, é **cadastrado**; se existe, faz **login**.

### Sessão com JWT + cookies (Aula 5)

- **Login grava um cookie e redireciona** — ao autenticar/cadastrar, o back-end
  gera um **JWT** assinado (HS256, válido por 10 min), guarda-o num **cookie
  `HttpOnly`** chamado `token` e redireciona para `/`.
- **Stateless** — não há sessão no servidor. O navegador reenvia o cookie
  automaticamente; o back-end **valida a assinatura** do token com a `SECRET_KEY`
  (só ele a conhece) e reconstrói o usuário. Token fabricado, adulterado ou
  expirado **não passa** na validação.
- **Extratores** — `User` (exige sessão válida) e `Option<User>` (tolerante:
  ausência/invalidez vira `None`, sem erro). A tela `index` usa o `Option<User>`:
  com sessão mostra `Hello <usuário>`; sem ela, **redireciona para `/login`** em
  vez de devolver um erro feio.

> ⚠️ **Didático, não pronto para produção.** Toda a autenticação foi feita "na
> mão" para desmistificar como JWT e cookies funcionam. Os segredos vêm do
> ambiente (ver `config.rs`), mas não há *refresh token* nem rotação de chave, e
> o JWT expira em 10 min sem renovação. Em um projeto real, prefira uma solução
> de autenticação consolidada.

### Carteira: saldo, posições e transações (Final)

- **Dinheiro exato com `rust_decimal`** — saldo, preços, quantidades e custo
  médio são `Decimal` (mapeado em `NUMERIC` no Postgres). Nada de `f64`: não há
  ruído de arredondamento de ponto flutuante em valores monetários.
- **`holdings` + `transactions`, não um log de compras** — em vez de derivar
  tudo de um histórico append-only, o esquema separa duas preocupações:
  - **`holdings`** — a **posição atual** por `(user, asset)`: `quantity` possuída
    e `avg_cost` (custo médio ponderado). Mutada atomicamente em cada compra/venda.
  - **`transactions`** — o **livro-razão imutável** de tudo o que aconteceu
    (`deposit`, `buy`, `sell`), usado para a tela de histórico e auditoria.
  Manter a posição materializada deixa as leituras triviais (sem agregação
  pesada) e torna a lógica de mover dinheiro um `UPDATE` transacional explícito.
- **Operações transacionais** (`repository.rs`):
  - `deposit` — credita o saldo e registra a transação.
  - `buy_asset` — trava o ativo e o usuário (`FOR UPDATE`), confere saldo, debita,
    abre/atualiza a posição recalculando o **custo médio ponderado** via
    `ON CONFLICT ... DO UPDATE`, e registra a transação. Tudo numa transação:
    saldo insuficiente reverte sem deixar rastro.
  - `sell_asset` — confere a posição, credita o saldo ao preço atual, reduz a
    quantidade (ou apaga a posição se zerar) e registra a transação.
  Cada uma é coberta por testes `#[sqlx::test]` (ver "Testes").
- **Resumo do patrimônio no banco** — `wallet_summary` calcula numa query só:
  saldo em caixa, valor das posições (`Σ quantidade × preço`), patrimônio total,
  total investido (`Σ quantidade × custo_médio`) e resultado dos ativos
  (`Σ quantidade × (preço − custo_médio)`).
- **Cotações de mercado reais** (`quotes.rs`) — "atualizar cotações" busca
  `USD→BRL` e `BTC→BRL` na API pública da Coinbase (concorrente via
  `tokio::try_join!`) e aplica os preços aos ativos cujo nome casa (bitcoin, btc,
  dólar, real…) em **um único `UPDATE` com `UNNEST`** (sem N+1).
- **Tela `/assets`** — busca resumo, posições, ativos disponíveis e transações
  em paralelo (`tokio::try_join!`), já que nenhuma query depende das outras. As
  operações (depositar/comprar/vender) abrem um formulário inline na própria tela.
- **Lucro/prejuízo colorido** — o filtro `nonnegative` decide entre verde
  (`text-emerald-400`, com prefixo `+`) e vermelho (`text-rose-400`), no resumo,
  por posição e nas transações.
- **Filtros Askama customizados** (`#[askama::filter_fn]`, módulo `filters` em
  `frontend.rs`): `human_datetime` (`AAAA-MM-DD HH:MM`), `money` (formata
  `Decimal` como `R$ 1.234,56`, com separador de milhar e duas casas),
  `quantity` (normaliza o `Decimal`, sem zeros à toa), `nonnegative` e
  `transaction_kind` (rótulo em pt-BR para `deposit`/`buy`/`sell`).
- **Estado vazio e acessibilidade** — *empty states* dedicados, `focus-visible`,
  `scope="col"`, `inputmode="decimal"`/`autocomplete` nos formulários, favicon
  SVG inline e `color-scheme: dark` no `base.html`.
- **Herança de templates** — `base.html` concentra o esqueleto comum; `login.html`
  e `assets.html` preenchem `title`/`content` com `{% extends "base.html" %}`.
- **Logout** — `GET /logout` remove o cookie `token` e redireciona para `/login`.
- **Index = roteador puro** — `/` com sessão vai para `/assets`; sem ela, `/login`.

## Rotas

### Front-end (HTML, na raiz)

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/health` | — | Sonda de saúde (200 se o serviço e o banco respondem) |
| `GET` | `/login` · `/register` | — | Formulário de login / cadastro |
| `POST` | `/login` · `/register` | — | Autentica ou cadastra; grava o cookie `token` e vai para `/` |
| `GET` | `/logout` | — | Remove o cookie `token` e redireciona para `/login` |
| `GET` | `/` | opcional | Roteador: com sessão vai para `/assets`, sem ela para `/login` |
| `GET` | `/assets` | sessão | Carteira: saldo, posições, resumo e transações |
| `POST` | `/deposit` | sessão | Deposita saldo (`amount`) |
| `POST` | `/buy` | sessão | Compra um ativo (`asset_id`, `quantity`) ao preço atual |
| `POST` | `/sell` | sessão | Vende um ativo (`asset_id`, `quantity`) ao preço atual |
| `POST` | `/quotes/sync` | sessão | Atualiza os preços com cotações de mercado |

### API do admin (JSON, sob `/api`)

Os de escrita exigem o header `Authorization: I'm the admin`.

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/api/assets` | — | Lista os ativos |
| `POST` | `/api/assets` | admin | Cadastra um ativo (`{name, unit_value}`) |
| `PATCH` | `/api/assets` | admin | Atualiza um ativo (`{id, name?, unit_value?}`) |

Erros: `400` header ausente / username já em uso, `401` credencial ou token
inválido, `404` ativo/usuário inexistente, `500` erro de banco ou template.

## Como rodar

Pré-requisitos: [Rust](https://rustup.rs), Docker (ou um Postgres próprio) e,
opcionalmente, `psql`.

```powershell
# 1) subir o Postgres
docker compose up -d

# 2) configurar a conexão (copie o exemplo, ajuste se precisar)
Copy-Item .env.example .env

# 3) criar as tabelas (todas as migrações de migrations/). Com a CLI do SQLx:
cargo install sqlx-cli --no-default-features --features postgres,rustls
cargo sqlx migrate run
#    Sem a CLI, aplique os arquivos *.up.sql de migrations/ em ordem com psql.

# 4) rodar (o Postgres precisa estar no ar — o SQLx checa as queries ao compilar)
cargo run
```

> O `DATABASE_URL` precisa estar disponível **na hora de compilar** (o `.env` já
> resolve isso, pois o SQLx o lê automaticamente). As credenciais do `.env`
> batem com o `docker-compose.yaml`.

### Usando o front-end

Abra <http://localhost:3000/login>, digite um usuário e senha e clique em
**entrar**:

- usuário **novo** → é cadastrado na hora (a mesma tela serve para login e
  cadastro), e você já entra;
- usuário **existente** → faz login validando a senha.

Depois de entrar você cai em `/assets`. Pode dar **F5** à vontade: a sessão
fica no cookie. Se o cookie `token` for removido ou adulterado, você é mandado
de volta para `/login`.

Na tela de ativos:

- cada ativo que você já comprou aparece num card, com a quantidade total, o
  valor unitário atual e o **lucro/prejuízo total** (verde com `+` se positivo,
  vermelho se negativo);
- clique em **histórico de compras** para expandir a tabela com cada compra
  individual (data, quantidade, valor pago e variação);
- clique em **registrar compra** para abrir o formulário (escolha o ativo, a
  quantidade e o valor unitário pago) — ao confirmar, a página recarrega com o
  novo total já recalculado;
- **sair** remove o cookie e volta para `/login`.

### Usando a API do admin

No PowerShell, prefira `Invoke-RestMethod` (ele já desserializa a resposta):

```powershell
$admin = @{ Authorization = "I'm the admin" }

Invoke-RestMethod http://127.0.0.1:3000/api/assets

Invoke-RestMethod -Method Post http://127.0.0.1:3000/api/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"name":"bitcoin","unit_value":10}'

Invoke-RestMethod -Method Patch http://127.0.0.1:3000/api/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"id":1,"unit_value":20}'
```

> Com `curl.exe`, use aspas **simples** no JSON e **não** escape as aspas com
> `\` (no PowerShell as aspas simples já são literais).

## Testes

```powershell
cargo test
```

Os testes usam `#[sqlx::test]` (precisa do Postgres no ar) e **insta** para os
snapshots. Para revisar/atualizar snapshots: `cargo install cargo-insta` e
`cargo insta review` — ou rode os testes uma vez com `$env:INSTA_UPDATE='always'`.

Cobertura: além dos snapshots da API do admin, o **núcleo financeiro** tem
testes dedicados em `repository.rs` (depósito, compra, venda, custo médio
ponderado, e as guardas de saldo/posição insuficientes), cada um num banco
efêmero isolado.

## Refinamentos de engenharia

Sobre a base do curso, alguns ajustes de gente grande:

- **Configuração centralizada e *fail-fast*** (`config.rs`) — os segredos são
  lidos e validados **uma vez** no boot. Falta um segredo? O serviço não sobe, com
  mensagem clara — em vez de devolver um `401` confuso na primeira requisição.
- **Sem releitura de ambiente por requisição** — `JWT_SECRET`, `ADMIN_SECRET_KEY`
  e `COOKIE_SECURE` ficam na `AppState`; validar token/credencial não toca mais o
  ambiente a cada chamada.
- **Erros 5xx não vazam detalhes** — falhas internas são logadas no servidor com a
  causa raiz e respondidas ao cliente com uma mensagem genérica (nada de texto de
  erro do SQL na resposta).
- **Startup robusto** — `.env` ausente não é fatal (produção usa o ambiente real),
  `RUST_LOG` controla o nível de log (`EnvFilter`), desligamento gracioso no
  Ctrl+C e endereço de escuta configurável (`BIND_ADDR`).
- **Sonda `/health`** — confirma serviço + banco para health checks de
  orquestradores.
- **Sem N+1** — a sincronização de cotações virou um único `UPDATE ... UNNEST`.

## Teste funcional

A sequência completa em funcionamento: subir o Postgres (`docker compose up`),
aplicar as migrações, rodar o servidor (`cargo run`) e exercer a API com
`Invoke-RestMethod` (GET/POST/PATCH), além da suíte de testes passando
(`cargo test` → 13 passed). _(A captura abaixo é de uma execução anterior, com a
suíte ainda menor.)_

![Teste funcional: docker compose up, migração, cargo run com logs do servidor e queries, requisições à API via Invoke-RestMethod e cargo test com 3 testes passando](assets/2026-06-02_23-18.png)

## Notas de ambiente (Windows)

- **TLS do cargo:** o download de dependências pode falhar com
  `CRYPT_E_NO_REVOCATION_CHECK`; o `.cargo/config.toml` já desativa só a checagem
  de revogação.
- **Postgres 18 no Docker:** o volume é montado em `/var/lib/postgresql` (e não
  mais em `/var/lib/postgresql/data`, convenção que mudou na imagem 18+).
- **`jwt-simple` sem `cmake`:** por padrão a lib usa BoringSSL, que exige `cmake`
  + toolchain C++. O `Cargo.toml` a configura com
  `default-features = false, features = ["pure-rust"]` para usar criptografia
  100% Rust e dispensar o `cmake`.

## Tecnologias

Em uso: **axum** (+ **axum-extra** para cookies), **tokio**, **sqlx** (Postgres,
compile-time checked), **askama** (templates SSR), **password-auth** (hash
argon2), **jwt-simple** (JWT), **dotenvy** (.env), **tracing** +
**tracing-subscriber**, **color-eyre**, **serde** / **serde_json**,
**thiserror**. Em testes: **insta** (snapshots).

## Cronograma

- **Aula 1 — Introdução** ✅: visão geral e fundação do repositório.
- **Aula 2 — Primeiros passos com Axum** ✅: API REST do admin, auth por *secret
  key*, armazenamento em memória e tratamento de erros.
- **Aula 3 — SQLx + Postgres** ✅: banco Postgres via SQLx, padrão repository,
  migrações em SQL e testes (`#[sqlx::test]` + insta).
- **Aula 4 — Primeira tela (Askama)** ✅: modelo de usuário no banco (senha com
  hash argon2), tela de login/cadastro renderizada no servidor.
- **Aula 5 — Autenticação stateless com JWT** ✅: sessão via JWT assinado em
  cookie HttpOnly; index que redireciona ao login quando não há sessão válida.
- **Final — Carteira completa** ✅: dinheiro exato com `rust_decimal`, esquema
  `holdings` + `transactions`, operações transacionais (depositar/comprar/vender)
  com custo médio ponderado, resumo do patrimônio e cotações de mercado reais
  (Coinbase). Acrescido de refinamentos de engenharia: configuração *fail-fast*,
  sonda `/health`, desligamento gracioso, censura de erros 5xx na resposta e
  testes do núcleo financeiro (ver "Refinamentos de engenharia").
