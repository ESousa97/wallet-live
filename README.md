# wallet :: restful stack

Carteira digital de investimentos construída **inteiramente em Rust** — backend
e frontend. Projeto do curso *RESTful Stack* (DIO), com o instrutor Breno Lemos.

> Status: **Aula 2 — API REST do admin** ✅
> O servidor expõe uma API JSON para cadastrar, listar e atualizar ativos, com
> autenticação por *secret key* e tratamento de erros. O armazenamento ainda é
> **em memória** (some ao reiniciar) — o banco de dados chega na Aula 3.

## Estrutura

```
src/
  main.rs          # enxuta: tokio::main -> App::start()
  app.rs           # App (inicialização) + AppState (estado compartilhado)
  models.rs        # Asset (modelo de domínio)
  error.rs         # AppError (enum) + IntoResponse com status HTTP correto
  auth/
    mod.rs
    admin.rs       # extrator Admin (autenticação por secret key)
  routes/
    mod.rs
    api.rs         # router() + handlers list/create/update
```

Pontos de destaque (tudo seguindo a Aula 2):

- **Estado compartilhado** — `AppState { assets: Arc<Mutex<HashMap<i64, Asset>>> }`.
  O `Arc<Mutex<…>>` deixa todas as rotas compartilharem o mesmo armazenamento
  mesmo sendo `Clone`. Usa a `Mutex` do **tokio** (assíncrona), não a da `std`.
- **Injeção de dependência** via extratores do Axum. O `Admin` implementa
  `FromRequestParts`: anotar um handler com um parâmetro `Admin` já o protege —
  sem credencial válida, o corpo do handler nem executa.
- **Tratamento de erros** — `AppError` (com `thiserror`) implementa
  `IntoResponse`, devolvendo o status HTTP adequado + um JSON `{"error": "..."}`.
- **Observabilidade** — `tracing` + `tracing-subscriber` logam no terminal;
  cada handler é anotado com `#[instrument(skip_all)]`.
- **Erros ergonômicos** — `color-eyre` na `main`/`start` para um `Result`
  amigável (`?` no lugar de `unwrap`).

## Endpoints

Todos sob o prefixo `/api`. Os de escrita exigem o header
`Authorization: I'm the admin`.

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/api/assets` | — | Lista os ativos |
| `POST` | `/api/assets` | admin | Cadastra um ativo (`{name, unit_value}`) |
| `PATCH` | `/api/assets` | admin | Atualiza um ativo (`{id, name?, unit_value?}`) |

Erros: `400` header de autorização ausente, `401` credencial inválida, `404`
ativo inexistente.

## Como rodar

Pré-requisitos: [Rust](https://rustup.rs) (toolchain estável). O servidor sobe
em <http://127.0.0.1:3000>.

```powershell
cargo run
```

No PowerShell, a forma mais confiável é o `Invoke-RestMethod` (sem as dores de
cabeça de aspas do `curl`); ele ainda já desserializa a resposta JSON:

```powershell
$admin = @{ Authorization = "I'm the admin" }

# listar
Invoke-RestMethod http://127.0.0.1:3000/api/assets

# cadastrar (admin)
Invoke-RestMethod -Method Post http://127.0.0.1:3000/api/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"name":"bitcoin","unit_value":10}'

# atualizar (admin)
Invoke-RestMethod -Method Patch http://127.0.0.1:3000/api/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"id":1,"unit_value":20}'
```

Com `curl.exe` também funciona (lembre que `curl` puro é um alias do
`Invoke-WebRequest`). Use aspas **simples** no JSON e **não** escape as aspas
internas com `\`: no PowerShell as aspas simples já são literais, então o `\`
iria parar no corpo e quebrar o JSON.

```powershell
curl.exe http://127.0.0.1:3000/api/assets

curl.exe -X POST http://127.0.0.1:3000/api/assets `
  -H "Authorization: I'm the admin" -H "Content-Type: application/json" `
  -d '{"name":"bitcoin","unit_value":10}'

curl.exe -X PATCH http://127.0.0.1:3000/api/assets `
  -H "Authorization: I'm the admin" -H "Content-Type: application/json" `
  -d '{"id":1,"unit_value":20}'
```

> **Windows / TLS:** o `cargo` pode falhar ao baixar dependências com o erro
> `CRYPT_E_NO_REVOCATION_CHECK`. Por isso o projeto inclui `.cargo/config.toml`
> com `check-revoke = false`, que desativa apenas essa checagem nos downloads.

## Tecnologias

Em uso agora: **axum** (web), **tokio** (runtime assíncrono), **tracing** +
**tracing-subscriber** (observabilidade), **color-eyre** (erros), **serde** /
**serde_json** (JSON), **thiserror** (enum de erros).

Chegam nas próximas aulas: **sqlx** (Postgres, Aula 3) e **askama** (templates
SSR, Aula 4).

## Cronograma

- **Aula 1 — Introdução** ✅: visão geral do projeto e fundação do repositório.
- **Aula 2 — Primeiros passos com Axum** ✅: API REST de cadastro, listagem e
  atualização de ativos (o admin), auth por *secret key*, armazenamento em
  memória e tratamento de erros.
- **Aula 3 — SQLx + Postgres**: troca do armazenamento em memória pelo banco,
  migrações em SQL e testes unitários (endpoint + banco).
- **Aula 4 — Primeira tela (Askama)**: modelo de usuário no banco; telas de
  login e cadastro e uma tela de índice simples.
- **Aula 5 — Autenticação stateless com JWT**: base para o usuário acessar a
  tela de ativos e registrar compras.
- **Final — Telas do usuário**: ver ativos comprados, ganhos/perdas, registrar
  novas compras e logout.
