# wallet :: restful stack

Carteira digital de investimentos construída **inteiramente em Rust** — backend
e frontend. Projeto do curso *RESTful Stack* (DIO), com o instrutor Breno Lemos.

> Status: **Aula 3 — SQLx + Postgres** ✅
> A API do admin agora persiste os ativos num banco **Postgres** via **SQLx**
> (queries checadas em tempo de compilação), organizada com o padrão
> **repository**, com migrações em SQL e testes (`#[sqlx::test]` + **insta**).

## Estrutura

```
src/
  main.rs          # enxuta: tokio::main -> App::start()
  app.rs           # App (inicialização) + AppState { db: PgPool }
  models.rs        # Asset (modelo de domínio)
  error.rs         # AppError + IntoResponse (status HTTP) + conversão de sqlx::Error
  repository.rs    # Repository: encapsula todo o acesso ao banco (queries)
  auth/admin.rs    # extrator Admin (autenticação por secret key)
  routes/
    api.rs         # router() + handlers + testes (#[sqlx::test] + insta)
    fixtures/      # dados SQL para testes (bitcoin_asset.sql)
    snapshots/     # snapshots aceitos do insta (.snap)
migrations/        # 20260602000000_create_assets.{up,down}.sql
docker-compose.yaml
.env / .env.example
```

Pontos de destaque (seguindo a Aula 3):

- **Banco no estado** — `AppState { db: PgPool }`. A `PgPool` é um `Arc` por
  dentro, então clonar o estado clona só o ponteiro, não as conexões.
- **Padrão repository** — `Repository` concentra todas as queries. Os handlers
  não sabem como o banco funciona, só que ele existe; mudou o esquema, muda só o
  repository. Ele também é um **extrator** do Axum (`FromRequestParts`,
  `Rejection = Infallible`), injetado direto nos endpoints.
- **Queries checadas em compilação** — `sqlx::query_as!` valida cada SQL contra
  o banco real **na hora de compilar**. Se a tabela/coluna não existir, o
  programa não compila. (Por isso o Postgres precisa estar no ar para buildar.)
- **Migrações** — versionadas em `migrations/`, com `up`/`down` reversíveis.
- **Erros** — `AppError` ganhou a variante `Database(#[from] sqlx::Error)` com
  `#[error(transparent)]`, mapeada para `500 Internal Server Error`.
- **Testes** — `#[sqlx::test]` cria um banco efêmero por teste, roda as migrações
  e aplica *fixtures*; o **insta** garante que o JSON de resposta não mude sem
  querer (snapshot testing).

## Endpoints

Todos sob `/api`. Os de escrita exigem o header `Authorization: I'm the admin`.

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/api/assets` | — | Lista os ativos |
| `POST` | `/api/assets` | admin | Cadastra um ativo (`{name, unit_value}`) |
| `PATCH` | `/api/assets` | admin | Atualiza um ativo (`{id, name?, unit_value?}`) |

Erros: `400` header ausente, `401` credencial inválida, `404` ativo inexistente,
`500` erro de banco.

## Como rodar

Pré-requisitos: [Rust](https://rustup.rs), Docker (ou um Postgres próprio) e,
opcionalmente, `psql`.

```powershell
# 1) subir o Postgres
docker compose up -d

# 2) configurar a conexão (copie o exemplo, ajuste se precisar)
Copy-Item .env.example .env

# 3) criar a tabela (uma migração). Com a CLI do SQLx:
#    cargo install sqlx-cli; cargo sqlx migrate run
#    Sem a CLI, aplicando o SQL direto:
psql "postgres://postgres:postgres@localhost:5432/postgres" -f migrations/20260602000000_create_assets.up.sql

# 4) rodar (o Postgres precisa estar no ar — o SQLx checa as queries ao compilar)
cargo run
```

> O `DATABASE_URL` precisa estar disponível **na hora de compilar** (o `.env` já
> resolve isso, pois o SQLx o lê automaticamente). As credenciais do `.env`
> batem com o `docker-compose.yaml`.

Exemplos de requisição (no PowerShell, prefira `Invoke-RestMethod` — ele já
desserializa a resposta):

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

## Tecnologias

Em uso: **axum**, **tokio**, **sqlx** (Postgres, compile-time checked),
**dotenvy** (.env), **tracing** + **tracing-subscriber**, **color-eyre**,
**serde** / **serde_json**, **thiserror**. Em testes: **insta** (snapshots).

Chega na próxima aula: **askama** (templates SSR, Aula 4).

## Cronograma

- **Aula 1 — Introdução** ✅: visão geral e fundação do repositório.
- **Aula 2 — Primeiros passos com Axum** ✅: API REST do admin, auth por *secret
  key*, armazenamento em memória e tratamento de erros.
- **Aula 3 — SQLx + Postgres** ✅: banco Postgres via SQLx, padrão repository,
  migrações em SQL e testes (`#[sqlx::test]` + insta).
- **Aula 4 — Primeira tela (Askama)**: modelo de usuário no banco; telas de
  login e cadastro e uma tela de índice simples.
- **Aula 5 — Autenticação stateless com JWT**: base para o usuário acessar a
  tela de ativos e registrar compras.
- **Final — Telas do usuário**: ver ativos comprados, ganhos/perdas, registrar
  novas compras e logout.
