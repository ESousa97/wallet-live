# Referência de endpoints

## Objetivo

Documentar cada rota HTTP do sistema: método, autenticação, autorização, parâmetros,
respostas possíveis, efeitos colaterais e evidência no código.

## Escopo

Coberto: as 21 rotas HTTP (16 de interface, 2 de API × 2 prefixos, 3 sondas). Não
coberto: o detalhe de cada campo de payload (ver [payloads.md](payloads.md)), o
mecanismo de autenticação (ver [authentication.md](authentication.md)) e o catálogo
de erros (ver [errors.md](errors.md)).

Cabeçalhos de segurança, cookies e convenções comuns a todas as respostas estão em
[api-overview.md](api-overview.md) §5 e não se repetem por rota.

---

## 1. Legenda de autenticação

| Marca | Significado |
| --- | --- |
| — | Pública, sem autenticação |
| **sessão** | Exige cookie `token` válido (ou renovação por `refresh_token`) |
| **opcional** | Funciona com e sem sessão, com comportamento diferente |
| **admin** | Exige papel `admin` na sessão **ou** header `Authorization` com `ADMIN_SECRET_KEY` |
| **CSRF** | Exige campo `csrf_token` batendo com o cookie `csrf` |

## 2. Interface do usuário (HTML, na raiz)

### 2.1 Navegação e sessão

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/` | opcional | Com sessão redireciona para `/assets`; sem, para `/login` |
| `GET` | `/login` | — | Formulário de login |
| `POST` | `/login` | CSRF | Autentica e grava os cookies de sessão |
| `GET` | `/register` | — | Formulário de cadastro |
| `POST` | `/register` | CSRF | Cadastra e já inicia sessão |
| `GET` | `/logout` | — | **Revoga a sessão no servidor** e remove os cookies |
| `GET` | `/lang/{code}` | — | Troca o idioma (`pt-BR`/`en`) e volta para `?next=` |

**`POST /login`** — corpo `application/x-www-form-urlencoded`:
`username`, `password`, `csrf_token`.

Sequência de verificação, **nesta ordem**:

1. `verify_csrf` — divergente ⇒ `403`.
2. `LoginThrottle::ensure_allowed` — em bloqueio ⇒ `429`. **Roda antes de conferir a
   senha**: durante o lockout, nem a senha correta passa.
3. Verificação de senha (argon2).

Credencial inválida e usuário inexistente produzem a **mesma** mensagem de flash — o
teste `business_errors_become_messages_and_internal_errors_do_not` trava isso, porque
mensagens diferentes vazariam quais contas existem.

Efeitos em caso de sucesso: `INSERT` em `sessions`, `Set-Cookie` de `token` e
`refresh_token`, redirecionamento `303` para `/assets`.

**`GET /logout`** — chama `revoke_session`, marcando `revoked_at` no banco. Não é
apenas apagar o cookie: a sessão morre no servidor.

**`GET /lang/{code}`** — o parâmetro `?next=` é validado contra **open redirect**:
só caminho local absoluto é aceito. Protocolo-relativo (`//site`), URL absoluta e
lixo caem no fallback `/`. Código de idioma desconhecido não grava cookie — só
redireciona.

> Sem essa validação, `/lang/pt-BR?next=https://site-falso` levaria o usuário para
> fora com um clique que parece do produto. Travado por
> `language_switch_only_follows_local_absolute_paths` e
> `the_language_switch_is_not_an_open_redirect`.

Evidência: `src/routes/frontend.rs` · `index`, `login_page`, `login`,
`register_page`, `register`, `logout`, `set_language`, `authenticate_form`.

### 2.2 Carteira e operações

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/assets` | sessão | Carteira: saldo, posições, resumo, gráfico e extrato |
| `GET` | `/deposit` · `/buy` · `/sell` | sessão | Carteira com o formulário da operação aberto |
| `POST` | `/deposit` | sessão + CSRF | Credita saldo |
| `POST` | `/buy` | sessão + CSRF | Compra ao preço atual do catálogo |
| `POST` | `/sell` | sessão + CSRF | Vende ao preço atual do catálogo |
| `POST` | `/quotes/sync` | sessão + CSRF | Dispara uma rodada de cotações |
| `GET` | `/transactions.csv` | sessão | Download do extrato completo |

**Parâmetro de query de `/assets`:**

| Parâmetro | Tipo | Obrigatório | Padrão | Descrição |
| --- | --- | :---: | --- | --- |
| `page` | `u32` | Não | 1 | Página do extrato, 25 por página (`TRANSACTIONS_PAGE_SIZE`) |

**Corpos dos formulários:**

| Rota | Campos |
| --- | --- |
| `POST /deposit` | `amount` (`Decimal`), `csrf_token` |
| `POST /buy` · `POST /sell` | `asset_id` (`i64`), `quantity` (`Decimal`), `csrf_token` |
| `POST /quotes/sync` | `csrf_token` |

**Resposta condicional a htmx** — as quatro rotas `POST` de operação respondem de
duas formas, a partir do **mesmo** handler:

| Requisição | Resposta |
| --- | --- |
| Com header `HX-Request` | Fragmento HTML atualizado + flash inline + `HX-Push-Url`, numa **única** resposta |
| Sem o header | `303` clássico (PRG), com o flash no cookie |
| Com `HX-History-Restore-Request` | **Página inteira** — voltar/avançar com cache expirado precisa reconstruir o DOM |

**Erros de negócio não são 5xx.** Saldo insuficiente, posição insuficiente, ativo
sem cotação ou total que arredonda a zero voltam como **banner acessível** no
formulário de origem, com `autofocus` no primeiro campo. Travado por
`a_business_error_comes_back_as_a_banner_not_a_500`.

**`POST /quotes/sync`** tem cooldown de **30 s** para chamadas manuais
(`AppError::QuoteSyncTooSoon` ⇒ `429`) e é serializado com o job agendado por
`Mutex`. Efeitos de uma rodada: `UPDATE` nos preços do catálogo (um único `UPDATE`,
sem N+1), criação do catálogo mínimo se estiver vazio, e um `INSERT` de snapshot de
patrimônio **por usuário**.

**`GET /transactions.csv`** — resposta com:

```text
Content-Type: text/csv; charset=utf-8
Content-Disposition: attachment; filename="extrato.csv"
```

Cabeçalho do arquivo:
`data;tipo;ativo;quantidade;preco_unitario;movimento_caixa`

Sem o `Content-Disposition`, o navegador renderizaria o CSV como texto na tela.
Convenção pt-BR de planilha: separador `;`, decimal com vírgula, aspas internas
dobradas.

Evidência: `src/routes/frontend.rs` · `assets_page`, `deposit_page`, `deposit`,
`buy_page`, `buy_asset`, `sell_page`, `sell_asset`, `sync_quotes`,
`transactions_csv`, `transactions_to_csv`, `wallet_outcome`, `render_wallet`.

### 2.3 Painel de mercado

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/market` | sessão | Painel da moeda selecionada + lista das 100 maiores |

| Parâmetro | Tipo | Obrigatório | Padrão | Descrição |
| --- | --- | :---: | --- | --- |
| `coin` | string | Não | primeira do ranking | Id da CoinGecko (`bitcoin`, `ethereum`…) |
| `range` | string | Não | `24h` | Janela do gráfico: `24h` ou `7d` |
| `q` | string | Não | — | Busca na lista lateral (ticker ou nome), normalizada e limitada a 32 caracteres |

**Esta rota nunca chama a API externa.** Lê só o snapshot em memória, então trocar de
moeda, de janela ou buscar **não custa nenhuma chamada** — a tela responde igual com
um ou mil acessos.

**Degradação de entrada é total: qualquer combinação de parâmetros responde `200`.**

| Entrada | Comportamento |
| --- | --- |
| `coin` inexistente | Cai na primeira do ranking |
| `range` inválido | Cai em `24h` |
| `q` com `&`, `=`, espaço | Percent-encoded nos links; termo normalizado e truncado em 32 |
| Snapshot vazio (antes da 1ª rodada) | `role="status"` com mensagem de carregamento |

Travado por `the_market_screen_accepts_any_state_in_the_query_string`, que exercita 7
combinações incluindo `%26%3D` — "parâmetro digitado à mão nunca pode virar 500".

Evidência: `src/routes/frontend.rs` · `market_page`, `MarketQuery`;
`src/market.rs` · `Market`, `Coin::matches`, `Range::from_tag`.

### 2.4 Assets estáticos

| Método | Rota | Auth | Content-Type |
| --- | --- | --- | --- |
| `GET` | `/static/app.css` | — | `text/css` |
| `GET` | `/static/htmx.js` | — | `application/javascript` |
| `GET` | `/static/money-input.js` | — | `application/javascript` |

Servidos **do próprio binário** via `include_str!` — nenhuma requisição a terceiro,
nenhum CDN.

**Política de cache**, e é a única exceção ao `no-store` global:

| Cabeçalho | Valor |
| --- | --- |
| `ETag` | Derivado do **conteúdo** |
| `Cache-Control` | `no-cache` (revalida sempre, não "não armazene") |
| Resposta a `If-None-Match` correspondente | `304`, **com corpo vazio** |

O motivo é registrado no teste: a URL é fixa e o conteúdo muda a cada build, então
com cache cego "o rebuild deixa HTML novo com CSS velho. Foi o que empilhou o painel
de mercado." Etiqueta fraca (`W/"..."`) e lista de etiquetas são tratadas.

Evidência: `src/routes/frontend.rs` · `app_css`, `htmx_js`, `money_input_js`;
testes `static_assets_revalidate_by_content_and_answer_304_when_unchanged` e
`static_assets_revalidate_instead_of_being_cached_blind`.

### 2.5 Comportamento sem sessão

Todas as rotas de dado privado — `/assets`, `/market`, `/transactions.csv`,
`/deposit`, `/buy`, `/sell` — respondem, para visitante anônimo:

| Origem | Resposta |
| --- | --- |
| Navegação normal | `303` para `/login` |
| Requisição htmx | Header **`HX-Redirect`** — redireciona o navegador **inteiro** |

O segundo caso é deliberado: renderizar a página de login **dentro** de um pedaço da
carteira é pior que um erro. Travado por
`an_expired_session_redirects_the_whole_browser_not_just_the_fragment` e
`private_screens_send_anonymous_visitors_to_the_login`.

## 3. API administrativa (JSON)

Montada em **dois** prefixos a partir do mesmo router: `/api/v1` (canônico) e `/api`
(alias de compatibilidade). As respostas são idênticas byte a byte, verificado por
teste.

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/api/v1/assets` | — | Lista o catálogo de ativos |
| `POST` | `/api/v1/assets` | **admin** | Cadastra um ativo |
| `PATCH` | `/api/v1/assets` | **admin** | Atualiza um ativo existente |
| `GET` | `/api/v1/openapi.json` | — | Especificação OpenAPI gerada do código |

### `GET /api/v1/assets`

Sem parâmetros. **Pública** — o catálogo de ativos e seus preços não é informação
privada de usuário.

```json
[
  { "id": 1, "name": "bitcoin", "unit_value": "327777.41000000" },
  { "id": 2, "name": "dólar",   "unit_value": "5.42000000" }
]
```

### `POST /api/v1/assets`

```json
{ "name": "ouro", "unit_value": "750.25" }
```

| Resposta | Quando |
| --- | --- |
| `200` + `Asset` | Criado |
| `400` | Nome vazio/em branco, preço negativo, JSON malformado, campo ausente, tipo trocado |
| `400` | Header `Authorization` ausente (`MissingAuthorization`) |
| `401` | Credencial inválida, ou sessão sem papel `admin` |
| `500` | Nome duplicado (viola `UNIQUE` em `assets.name`) — ver observação abaixo |

> **Observação verificada:** nome duplicado em `assets` **não** tem tratamento
> específico e vira `AppError::Database` ⇒ `500`. Contraste com `users.username`, que
> tem tradução dedicada para `UsernameTaken` ⇒ `400`. Registrado como débito técnico
> **DT-06** em [../decisions/technical-debt.md](../decisions/technical-debt.md).

Efeitos: `INSERT` em `assets`, com nome **trimado** e preço arredondado para
`MONEY_SCALE`.

### `PATCH /api/v1/assets`

Atualização parcial: só os campos enviados são alterados.

```json
{ "id": 1, "unit_value": "760.10" }
```

| Resposta | Quando |
| --- | --- |
| `200` + `Asset` | Atualizado |
| `400` | Entrada inválida |
| `401` | Credencial inválida |
| `404` | `id` inexistente |

O `404` é explícito (`AppError::AssetDoesNotExist`), não `200` silencioso — "200 para
um id inexistente faria o operador achar que corrigiu". Em caso de entrada inválida,
o ativo fica **intocado**.

### Exemplo completo (PowerShell)

```powershell
$admin = @{ Authorization = $env:ADMIN_SECRET_KEY }

Invoke-RestMethod http://127.0.0.1:3000/api/v1/assets

$asset = Invoke-RestMethod -Method Post http://127.0.0.1:3000/api/v1/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"name":"ouro","unit_value":750.25}'

Invoke-RestMethod -Method Patch http://127.0.0.1:3000/api/v1/assets -Headers $admin `
  -ContentType 'application/json' `
  -Body (@{ id = $asset.id; unit_value = 760.10 } | ConvertTo-Json)
```

> Os exemplos leem a credencial de `$env:ADMIN_SECRET_KEY`. **Nunca** cole a
> credencial literal num comando: ela fica no histórico do shell.

Evidência: `src/routes/api.rs` · `router`, `list_assets`, `create_asset`,
`update_asset`, `CreateAssetRequest`, `UpdateAssetRequest`, `ApiDoc`;
`src/repository.rs` · `list_assets`, `create_asset`, `update_asset`.

## 4. Sondas de saúde

| Método | Rota | Toca o banco? | `200` | Falha | Ação esperada do orquestrador |
| --- | --- | :---: | --- | --- | --- |
| `GET` | `/healthz` | **Não** | Sempre | — | **Reiniciar** o container |
| `GET` | `/readyz` | Sim (`SELECT 1`) | Banco respondendo | `503` | **Tirar do balanceador**, sem reiniciar |
| `GET` | `/health` | Sim | Alias de `/readyz` | `503` | Idem |

A separação é operacional, não estética: **reiniciar o app não conserta um Postgres
fora do ar**. Uma liveness que dependesse do banco entraria em laço de reinício
durante uma indisponibilidade de banco, piorando o incidente.

`/health` é alias histórico da **readiness** — não da liveness.

Nenhuma das três exige autenticação nem revela informação de estado além do status
HTTP.

Evidência: `src/app.rs` · `liveness`, `readiness`, `App::router`; teste
`liveness_and_readiness_are_separate_probes`.

## 5. Tabela-resumo de todas as rotas

| # | Método | Rota | Auth | CSRF | Escreve? |
| --: | --- | --- | --- | :---: | :---: |
| 1 | `GET` | `/` | opcional | — | — |
| 2 | `GET` | `/healthz` | — | — | — |
| 3 | `GET` | `/readyz` | — | — | — |
| 4 | `GET` | `/health` | — | — | — |
| 5 | `GET` | `/login` | — | — | — |
| 6 | `POST` | `/login` | — | Sim | `sessions` |
| 7 | `GET` | `/register` | — | — | — |
| 8 | `POST` | `/register` | — | Sim | `users`, `sessions` |
| 9 | `GET` | `/logout` | — | — | `sessions` |
| 10 | `GET` | `/assets` | sessão | — | — |
| 11 | `GET` | `/market` | sessão | — | — |
| 12 | `GET` | `/transactions.csv` | sessão | — | — |
| 13 | `GET` | `/deposit` | sessão | — | — |
| 14 | `POST` | `/deposit` | sessão | Sim | `users`, `transactions` |
| 15 | `GET` | `/buy` | sessão | — | — |
| 16 | `POST` | `/buy` | sessão | Sim | `users`, `holdings`, `transactions` |
| 17 | `GET` | `/sell` | sessão | — | — |
| 18 | `POST` | `/sell` | sessão | Sim | `users`, `holdings`, `transactions` |
| 19 | `POST` | `/quotes/sync` | sessão | Sim | `assets`, `portfolio_snapshots` |
| 20 | `GET` | `/lang/{code}` | — | — | — |
| 21 | `GET` | `/static/{app.css,htmx.js,money-input.js}` | — | — | — |
| 22 | `GET` | `/api/v1/assets` · `/api/assets` | — | — | — |
| 23 | `POST` | `/api/v1/assets` · `/api/assets` | **admin** | — | `assets` |
| 24 | `PATCH` | `/api/v1/assets` · `/api/assets` | **admin** | — | `assets` |
| 25 | `GET` | `/api/v1/openapi.json` · `/api/openapi.json` | — | — | — |

**A API JSON não exige CSRF** porque não é chamada por formulário de navegador. A
proteção equivalente vem do `SameSite=Strict` do cookie de sessão, que impede outro
site de usar a sessão da vítima — registrado como nota em `src/auth/admin.rs`.

## 6. Evidências

```text
- src/app.rs             · App::router, liveness, readiness
- src/routes/frontend.rs · router (16 rotas) e todos os handlers
- src/routes/api.rs      · router (2 rotas + spec)
- tests/http_web.rs      (15 testes pelo router real)
- tests/http_api.rs      (8 testes pelo router real)
```
