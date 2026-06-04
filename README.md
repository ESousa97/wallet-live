# wallet :: restful stack

Carteira digital de investimentos construída **inteiramente em Rust** — backend
e frontend. Projeto do curso *RESTful Stack* (DIO), com o instrutor Breno Lemos.

> Status: **Aula 5 — Autenticação stateless com JWT** ✅
> Além da API do admin (Postgres + SQLx), o sistema agora tem **usuários** no
> banco (senha com hash **argon2**), uma **tela de login/cadastro** renderizada no
> servidor com **Askama**, e **sessão stateless** via **JWT** assinado guardado
> num **cookie HttpOnly** — o login sobrevive a um F5.

## Estrutura

```
src/
  main.rs            # enxuta: tokio::main -> App::start()
  app.rs             # App (inicialização) + AppState { db: PgPool }; monta os routers
  models.rs          # Asset (modelo de domínio) + UserRecord (linha crua de users)
  error.rs           # AppError + IntoResponse (status HTTP) + conversões de erro
  repository.rs      # Repository: encapsula todo o acesso ao banco (queries)
  auth/
    admin.rs         # extrator Admin (autenticação por secret key, p/ a API)
    user.rs          # User/UnauthenticatedUser, hash de senha, JWT e extratores
  routes/
    api.rs           # API REST do admin (JSON) + testes (#[sqlx::test] + insta)
    frontend.rs      # front-end SSR (HTML): login/cadastro e index
    fixtures/        # dados SQL para testes (bitcoin_asset.sql)
    snapshots/       # snapshots aceitos do insta (.snap)
templates/
  login.html         # template Askama da tela de login (Tailwind via CDN)
migrations/          # create_assets + create_users ({up,down}.sql)
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

## Rotas

### Front-end (HTML, na raiz)

| Método | Rota | Descrição |
| --- | --- | --- |
| `GET` | `/login` | Formulário de login/cadastro |
| `POST` | `/login` | Autentica **ou** cadastra; grava o cookie `token` e redireciona para `/` |
| `GET` | `/` | Index: saúda o usuário logado, ou redireciona para `/login` |

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

# 3) criar as tabelas (duas migrações). Com a CLI do SQLx:
#    cargo install sqlx-cli; cargo sqlx migrate run
#    Sem a CLI, aplicando o SQL direto:
psql "postgres://postgres:postgres@localhost:5432/postgres" -f migrations/20260602000000_create_assets.up.sql
psql "postgres://postgres:postgres@localhost:5432/postgres" -f migrations/20260603000000_create_users.up.sql

# 4) rodar (o Postgres precisa estar no ar — o SQLx checa as queries ao compilar)
cargo run
```

> O `DATABASE_URL` precisa estar disponível **na hora de compilar** (o `.env` já
> resolve isso, pois o SQLx o lê automaticamente). As credenciais do `.env`
> batem com o `docker-compose.yaml`.

### Usando o front-end

Abra <http://localhost:3000/login>, digite um usuário e senha e clique em
**Entrar**:

- usuário **novo** → é cadastrado na hora (a mesma tela serve para login e
  cadastro), e você já entra;
- usuário **existente** → faz login validando a senha.

Depois de entrar você cai em `/`, que mostra `Hello <usuário>`. Pode dar **F5**
à vontade: a sessão fica no cookie. Se o cookie `token` for removido ou
adulterado, você é mandado de volta para `/login`.

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
- **Final — Telas do usuário**: ver ativos comprados, ganhos/perdas, registrar
  novas compras e logout.
