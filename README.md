# wallet :: restful stack

Carteira digital de investimentos construída **inteiramente em Rust** — backend
e frontend. Projeto do curso *RESTful Stack* (DIO), com o instrutor Breno Lemos.

> Status: **Final — Telas do usuário** ✅
> Carteira completa: além da API do admin e da sessão stateless via JWT, o
> usuário agora tem uma **tela de ativos** (`/assets`) que mostra o que ele
> possui, **lucro/prejuízo** por ativo e o **histórico de compras**, com um
> formulário (em `<dialog>`) para **registrar novas compras** — e **logout**.

## Estrutura

Crate: **`wallet`** (nome curto, sem traço). A UI segue a estética da demo do
curso: textos em inglês minúsculo e fonte monoespaçada.

```
src/
  main.rs            # enxuta: tokio::main -> App::start()
  app.rs             # App (inicialização) + AppState { db: PgPool }; monta os routers
  models.rs          # Asset, UserRecord, OwnedAssetRecord, PurchaseRecord, OwnedAsset
  error.rs           # AppError + IntoResponse (status HTTP) + conversões de erro
  repository.rs      # Repository: encapsula todo o acesso ao banco (queries)
  auth/
    admin.rs         # extrator Admin (autenticação por secret key, p/ a API)
    user.rs          # User/UnauthenticatedUser, hash de senha, JWT e extratores
  routes/
    api.rs           # API REST do admin (JSON) + testes (#[sqlx::test] + insta)
    frontend.rs      # front-end SSR (HTML): login/logout, index, ativos e compras
    fixtures/        # dados SQL para testes (bitcoin_asset.sql)
    snapshots/       # snapshots aceitos do insta (.snap)
templates/
  base.html          # esqueleto comum (head, Tailwind, fonte mono); as páginas o estendem
  login.html         # tela de login ({% extends "base.html" %})
  assets.html        # tela de ativos: ganhos/perdas, histórico e compra ({% extends "base.html" %})
migrations/          # create_assets + create_users + create_owned_assets ({up,down}.sql)
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
> mão" para desmistificar como JWT e cookies funcionam. A `SECRET_KEY` está no
> código (em produção viria de variável de ambiente/cofre de segredos), não há
> *refresh token*, e os erros internos não são censurados na resposta. Em um
> projeto real, prefira uma solução de autenticação consolidada.

### Ativos do usuário, lucro/prejuízo e histórico de compras (Final)

- **Tabela `owned_assets`** — o histórico de compras: cada linha é "o usuário
  `user_id` comprou `quantity` unidades do ativo `asset_id` por `unit_value`
  cada, em `bought_at`" (FK para `users` e `assets`; `id`/`bought_at` gerados
  pelo banco via `BIGSERIAL`/`DEFAULT NOW()`).
- **Agregação no banco, não no Rust** — `list_owned_assets` faz `JOIN` entre
  `assets` e `owned_assets`, agrupando por ativo com `GROUP BY`. Para cada
  ativo o Postgres calcula a soma da quantidade possuída (`quantity_owned`), o
  lucro/prejuízo total (`value_delta`, = `(valor atual - valor pago) *
  quantidade`, somado por compra) e monta o **histórico de compras como JSON**
  com `JSON_AGG`/`JSON_BUILD_OBJECT` (ordenado da compra mais recente para a
  mais antiga).
- **`sqlx::types::Json<Vec<PurchaseRecord>>`** — o SQLx decodifica a coluna
  JSON agregada direto numa `Vec<PurchaseRecord>`. As colunas de `SUM`/JSON
  usam o sufixo `!` (`AS "value_delta!"`, `AS "purchase_history!:
  Json<Vec<PurchaseRecord>>"`) para o SQLx confiar que não são `NULL` — o
  `JOIN` garante pelo menos uma linha por grupo.
- **`OwnedAsset` vs. `PurchaseRecord`** — `OwnedAsset` é o resumo por ativo
  (o que aparece no card); `PurchaseRecord` é uma compra dentro do histórico
  (`bought_at`, `unit_value`, `quantity`, `value_delta` daquela compra
  específica). `PurchaseRecord::bought_at` usa `time::serde::rfc3339`, porque
  o Postgres serializa `timestamptz` em JSON nesse formato.
- **Tela `/assets`** — concorrente via `tokio::try_join!`: busca os ativos que
  o usuário possui (`list_owned_assets`) e os ativos disponíveis no sistema
  (`list_assets`, para popular o formulário) ao mesmo tempo, já que uma query
  não depende da outra.
- **Resumo do portfólio** — acima dos cards, uma barra com **valor atual**,
  **investido** e **lucro/prejuízo total**, agregados sobre todas as posições.
  Os três totais são calculados no `assets_page` (não no template): `value` é
  `Σ preço_atual × quantidade`, `delta` é `Σ value_delta`, e `invested = value
  − delta`. A barra só aparece quando há ativos.
- **Lucro/prejuízo colorido** — no template, `value_delta >= 0.0` decide entre
  verde (`text-emerald-400`, com prefixo `+`) e vermelho (`text-rose-400`), no
  total do portfólio, no total por ativo e em cada linha do histórico.
- **Filtros Askama customizados** (`#[askama::filter_fn]`, módulo `filters` em
  `frontend.rs`):
  - `human_datetime` formata `OffsetDateTime` como `AAAA-MM-DD HH:MM`;
  - `money` formata `f64` com **duas casas fixas** (`{:.2}`). É o que esconde o
    ruído de arredondamento de float que o professor mencionou na demo
    (`-9.999999999998` vira `-10.00`); valores monetários usam ainda
    `tabular-nums` para alinhar as colunas numéricas.
- **Estado vazio** — usuário sem nenhuma compra vê um *empty state* dedicado
  (com um CTA "register purchase") em vez de uma página em branco.
- **Acessibilidade e detalhes** — `focus-visible` em todos os botões/links,
  `scope="col"` nos cabeçalhos da tabela, `aria-labelledby` no `<dialog>`,
  `autofocus`/`autocomplete`/`inputmode="decimal"` nos formulários, e um
  favicon SVG inline + `color-scheme: dark` no `base.html`.
- **Quase zero JavaScript** — o histórico de compras é um `<details>`/
  `<summary>` (expande sem JS); o `<dialog>` de compra abre com um único
  `onclick="…showModal()"` e o botão **cancel** fecha via `formmethod="dialog"`
  (recurso nativo do form-in-dialog) — sem `close()` manual. Não há nenhum
  `<script>` próprio na página.
- **Herança de templates** — `base.html` concentra o esqueleto comum (head,
  Tailwind, fonte mono e tema escuro); `login.html` e `assets.html` só
  preenchem os blocos `title` e `content` com `{% extends "base.html" %}` —
  o mecanismo de *extends* do Askama, igual ao do Jinja2/Django templates.
- **Logout** — `GET /logout` remove o cookie `token` (`CookieJar::remove`) e
  redireciona para `/login`.
- **Index = roteador puro** — `/` não renderiza mais nada: com sessão válida
  redireciona para `/assets`; sem ela, para `/login`.

## Rotas

### Front-end (HTML, na raiz)

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/login` | — | Formulário de login/cadastro |
| `POST` | `/login` | — | Autentica **ou** cadastra; grava o cookie `token` e redireciona para `/` |
| `GET` | `/logout` | — | Remove o cookie `token` e redireciona para `/login` |
| `GET` | `/` | opcional | Roteador: com sessão vai para `/assets`, sem ela para `/login` |
| `GET` | `/assets` | sessão | Ativos possuídos (lucro/prejuízo + histórico) e formulário de compra |
| `POST` | `/assets` | sessão | Registra uma compra (`asset_id`, `quantity`, `unit_value`) e redireciona para `/assets` |

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

# 3) criar as tabelas (três migrações). Com a CLI do SQLx:
#    cargo install sqlx-cli; cargo sqlx migrate run
#    Sem a CLI, aplicando o SQL direto:
psql "postgres://postgres:postgres@localhost:5432/postgres" -f migrations/20260602000000_create_assets.up.sql
psql "postgres://postgres:postgres@localhost:5432/postgres" -f migrations/20260603000000_create_users.up.sql
psql "postgres://postgres:postgres@localhost:5432/postgres" -f migrations/20260604000000_create_owned_assets.up.sql

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

## Teste funcional

A sequência completa em funcionamento: subir o Postgres (`docker compose up`),
aplicar a migração, rodar o servidor (`cargo run`) e exercer a API com
`Invoke-RestMethod` (GET/POST/PATCH), além da suíte de testes passando
(`cargo test` → 3 passed).

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
- **Final — Telas do usuário** ✅: tabela `owned_assets`, agregação no banco
  (`JOIN` + `GROUP BY` + `JSON_AGG`/`JSON_BUILD_OBJECT`) para lucro/prejuízo e
  histórico de compras por ativo, tela `/assets` (ativos possuídos + formulário
  de compra num `<dialog>`), filtro Askama `human_datetime` e `/logout`.
