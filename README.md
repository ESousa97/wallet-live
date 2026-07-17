# wallet

Carteira digital de investimentos escrita **inteiramente em Rust** — backend e
frontend servidos pelo mesmo binário. API REST administrativa (JSON) e interface
do usuário renderizada no servidor (SSR), com **valores monetários exatos**,
operações **transacionais** e cotações de mercado reais.

## Funcionalidades

- **Carteira completa** — saldo em caixa, depósito, compra e venda de ativos ao
  preço de mercado, custo médio ponderado por posição, lucro/prejuízo por ativo
  e resumo do patrimônio.
- **Extrato** — livro-razão imutável de transações (depósitos, compras, vendas).
- **Cotações reais** — sincronização de preços (`USD→BRL`, `BTC→BRL`) via API
  pública da Coinbase, aplicada em um único `UPDATE` (sem N+1).
- **Autenticação e sessão** — cadastro/login com hash de senha (argon2); sessão
  com **JWT de acesso curto** + **refresh token rotacionado e revogável**
  (logout mata a sessão no servidor), ambos em cookies `HttpOnly` +
  `SameSite=Strict`; **lockout progressivo** contra força bruta e **CSRF
  tokens** em todos os formulários.
- **API administrativa** — catálogo de ativos sob `/api/v1`, autorizada por
  **papel de usuário** (sessão de admin) ou por credencial de serviço com
  comparação em tempo constante.
- **Pronto para orquestração** — sonda `/health` (serviço + banco), desligamento
  gracioso e logs estruturados (`tracing`).

## Decisões de engenharia

| Tema | Decisão |
| --- | --- |
| Dinheiro | `rust_decimal::Decimal` ↔ `NUMERIC` no Postgres. Ponto flutuante nunca toca valor monetário. |
| Consistência | Compra/venda/depósito rodam em transação com `FOR UPDATE`; saldo insuficiente reverte tudo. O schema tem `CHECK`s (saldo, preço e quantidade não negativos) como última linha de defesa. |
| Modelo de dados | `holdings` materializa a posição atual por (usuário, ativo); `transactions` é o histórico imutável. Leituras triviais, escrita explícita. |
| SQL | `sqlx::query_as!` — toda query é **checada em tempo de compilação** contra o banco. Schema divergente = não compila. |
| Injeção de dependência | Extratores do Axum (`Repository`, `User`, `Admin`): a assinatura do handler declara o que ele exige; sem satisfazer, o handler nem roda. |
| Sessão | JWT de acesso curto (stateless) + refresh token opaco com rotação a cada uso e hash SHA-256 no banco — revogável de verdade, replay de token queimado não funciona. Renovação transparente via middleware. |
| Defesas HTTP | CSRF *double-submit* nos formulários, lockout com backoff no login, CSP + `nosniff` + `X-Frame-Options` + `Referrer-Policy` em toda resposta, HSTS atrás de HTTPS. |
| Erros | Enum único (`AppError`) mapeado para status HTTP corretos; falhas 5xx são logadas com causa raiz e respondidas com mensagem genérica (nada de detalhe interno na resposta). |
| Configuração | Lida e validada **uma vez** no boot (*fail-fast*): segredo ausente derruba o serviço com mensagem clara, não um 401 confuso em produção. |
| Templates | Askama — variáveis dos templates também checadas em compilação. |

## Estrutura

```
src/
  main.rs            # enxuta: tokio::main -> App::start()
  app.rs             # boot, AppState { db, config }, /health, shutdown gracioso
  config.rs          # Config: lê e valida o ambiente uma vez (fail-fast)
  models.rs          # Asset, UserRecord, WalletSummary, Holding, Transaction
  error.rs           # AppError + IntoResponse (status HTTP, censura de 5xx)
  quotes.rs          # cotações de mercado (Coinbase) -> preços dos ativos
  repository.rs      # todo o acesso ao banco (queries + transações) + testes
  auth/
    admin.rs         # extrator Admin (sessão com role admin OU credencial de serviço)
    user.rs          # User/UnauthenticatedUser, hash de senha, JWT, extratores
    session.rs       # refresh token (rotação/revogação) + middleware de renovação
    csrf.rs          # proteção CSRF (double-submit cookie)
    throttle.rs      # lockout progressivo de login
  services/
    portfolio.rs     # PortfolioService: visão da carteira + operações (regra de negócio)
  routes/
    api.rs           # API REST administrativa (JSON) + OpenAPI + testes de snapshot
    frontend.rs      # SSR: login/logout, carteira, operações, filtros Askama
templates/           # base.html (esqueleto) + login.html + assets.html
migrations/          # schema versionado, up/down reversíveis
```

## Rotas

### Interface do usuário (HTML, na raiz)

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/health` | — | Sonda de saúde (200 se serviço e banco respondem) |
| `GET` | `/login` · `/register` | — | Formulários de login / cadastro |
| `POST` | `/login` · `/register` | — | Autentica ou cadastra; grava o cookie de sessão |
| `GET` | `/logout` | — | Revoga a sessão no servidor e remove os cookies |
| `GET` | `/` | opcional | Com sessão vai para `/assets`; sem, para `/login` |
| `GET` | `/assets` | sessão | Carteira: saldo, posições, resumo e extrato (paginado via `?page=`) |
| `POST` | `/deposit` | sessão | Deposita saldo (`amount`) |
| `POST` | `/buy` | sessão | Compra um ativo (`asset_id`, `quantity`) ao preço atual |
| `POST` | `/sell` | sessão | Vende um ativo (`asset_id`, `quantity`) ao preço atual |
| `POST` | `/quotes/sync` | sessão | Atualiza os preços com cotações de mercado |

### API administrativa (JSON, sob `/api/v1` — `/api` mantido como alias)

Escritas exigem **sessão de um usuário com papel `admin`** ou o header de
serviço `Authorization: <ADMIN_SECRET_KEY>`.

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/api/v1/assets` | — | Lista os ativos |
| `POST` | `/api/v1/assets` | admin | Cadastra um ativo (`{name, unit_value}`) |
| `PATCH` | `/api/v1/assets` | admin | Atualiza um ativo (`{id, name?, unit_value?}`) |
| `GET` | `/api/v1/openapi.json` | — | Especificação OpenAPI gerada do código |

Erros: `400` entrada inválida (header ausente, nome vazio, preço negativo,
quantia não positiva, saldo/posição insuficiente, username em uso), `401`
credencial ou token inválido, `403` token CSRF ausente/divergente, `404`
recurso inexistente, `429` lockout por excesso de tentativas de login, `502`
cotação indisponível, `500` falha interna (detalhes apenas no log do servidor).

## Configuração

Variáveis de ambiente (ver `.env.example`):

| Variável | Obrigatória | Descrição |
| --- | --- | --- |
| `DATABASE_URL` | sim | Conexão com o Postgres |
| `ADMIN_SECRET_KEY` | sim | Credencial da API administrativa |
| `JWT_SECRET` | sim | Chave de assinatura dos tokens de sessão |
| `COOKIE_SECURE` | não (`false`) | Marca os cookies como `Secure` e liga o HSTS (use `true` atrás de HTTPS) |
| `BIND_ADDR` | não (`0.0.0.0:3000`) | Endereço/porta de escuta |
| `SESSION_TTL_MINUTES` | não (`10`) | Validade do token de acesso |
| `REFRESH_TTL_DAYS` | não (`14`) | Validade do refresh token (sessão no servidor) |
| `RUST_LOG` | não (`info`) | Nível de log (ex.: `wallet=debug,info`) |

## Como rodar

Pré-requisitos: [Rust](https://rustup.rs) e Docker (ou um Postgres próprio).

```powershell
# 1) subir o Postgres
docker compose up -d

# 2) configurar o ambiente (copie o exemplo e ajuste os segredos)
Copy-Item .env.example .env

# 3) aplicar as migrações
cargo install sqlx-cli --no-default-features --features postgres,rustls
cargo sqlx migrate run

# 4) rodar (o Postgres precisa estar no ar: o SQLx valida as queries ao compilar)
cargo run
```

Abra <http://localhost:3000>, cadastre um usuário e use a carteira: deposite,
compre/venda ativos e sincronize as cotações. A sessão persiste no cookie;
token removido, adulterado ou expirado leva de volta ao login.

### Exemplo de uso da API administrativa

```powershell
$admin = @{ Authorization = $env:ADMIN_SECRET_KEY }

Invoke-RestMethod http://127.0.0.1:3000/api/v1/assets

Invoke-RestMethod -Method Post http://127.0.0.1:3000/api/v1/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"name":"bitcoin","unit_value":10}'

Invoke-RestMethod -Method Patch http://127.0.0.1:3000/api/v1/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"id":1,"unit_value":20}'
```

## Testes

```powershell
cargo test
```

- `#[sqlx::test]` cria um **banco efêmero por teste** (migrações aplicadas
  automaticamente), então os testes são isolados e paralelos.
- O **núcleo financeiro** tem cobertura dedicada: depósito, compra, venda,
  custo médio ponderado, guardas de saldo/posição insuficientes e validação de
  entradas.
- O contrato JSON da API é congelado com **insta** (snapshot testing):
  `cargo insta review` para auditar mudanças de formato.

## Roadmap

O plano de evolução — segurança de sessão, camada de serviço, CI/CD,
observabilidade e novas funcionalidades — está em [ROADMAP.md](ROADMAP.md).

## Tecnologias

**axum** (+ axum-extra), **tokio**, **sqlx** (Postgres, compile-time checked),
**askama**, **rust_decimal**, **password-auth** (argon2), **jwt-simple**,
**subtle**, **reqwest**, **tracing**, **thiserror**, **color-eyre**, **serde**,
**utoipa** (OpenAPI). Em testes: **insta**.

## Notas de ambiente (Windows)

- **TLS do cargo:** o download de dependências pode falhar com
  `CRYPT_E_NO_REVOCATION_CHECK`; o `.cargo/config.toml` já desativa só a checagem
  de revogação.
- **Postgres 18 no Docker:** o volume é montado em `/var/lib/postgresql`
  (convenção da imagem 18+).
- **`jwt-simple` sem `cmake`:** configurado com `pure-rust` para dispensar
  BoringSSL/cmake.
