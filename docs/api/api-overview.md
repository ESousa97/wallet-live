# Visão geral das interfaces

## Objetivo

Inventariar **todas** as superfícies de comunicação do sistema — HTTP de entrada,
chamadas de saída, cookies, jobs internos e arquivos de configuração — e apontar,
para cada uma, o documento que a detalha.

## Escopo

Coberto: o inventário completo de contratos, a política de versionamento e as
convenções comuns a todas as respostas. Não coberto: o detalhe campo a campo (ver
[endpoints.md](endpoints.md) e [payloads.md](payloads.md)), autenticação (ver
[authentication.md](authentication.md)) e erros (ver [errors.md](errors.md)).

---

## 1. Inventário de superfícies de comunicação

O sistema tem **menos** superfícies do que um checklist genérico esperaria, e
registrar as ausências é parte da documentação:

| Superfície | Existe? | Onde |
| --- | --- | --- |
| HTTP de entrada — interface HTML (SSR) | **Sim**, 16 rotas | [endpoints.md](endpoints.md) §2 |
| HTTP de entrada — API REST JSON | **Sim**, 2 rotas × 2 prefixos + spec | [endpoints.md](endpoints.md) §3 |
| HTTP de entrada — sondas de saúde | **Sim**, 3 rotas | [endpoints.md](endpoints.md) §4 |
| HTTP de saída — integrações externas | **Sim**, 2 | [payloads.md](payloads.md) §4 e §5 |
| Cookies | **Sim**, 5 | §3 abaixo |
| Jobs em segundo plano | **Sim**, 2 | §4 abaixo |
| Arquivos de configuração | **Sim** (ambiente, `.env`) | [../getting-started/configuration.md](../getting-started/configuration.md) |
| Exportação de telemetria | **Sim**, OTLP/HTTP (opt-in) | [../operations/observability.md](../operations/observability.md) |
| WebSocket | **Não** | A interatividade é htmx sobre HTTP ([ADR-0003](../adr/0003-ssr-com-askama-e-htmx.md)) |
| Server-Sent Events | **Não** | — |
| UDP / TCP bruto | **Não** | Só HTTP e a conexão SQL |
| IPC / chamadas nativas / FFI | **Não** | Um único processo, sem FFI |
| Filas de mensagem | **Não** | Os jobs usam `tokio::interval`, não fila |
| Webhooks (entrada ou saída) | **Não** | — |
| Eventos internos / barramento | **Não** | Comunicação entre módulos é chamada de função direta |
| Comandos remotos / agentes | **Não** | — |
| GraphQL / gRPC | **Não** | — |

## 2. Versionamento

| Aspecto | Estado |
| --- | --- |
| Caminho canônico da API | `/api/v1` |
| Alias de compatibilidade | `/api` — serve o **mesmo** router, verificado byte a byte por teste |
| Versão declarada na spec | `1.0.0` (`ApiDoc.info.version` em `src/routes/api.rs`) |
| Interface HTML | **Não versionada** — não há consumidor programático |
| Política de descontinuação | **Não definida.** Ver [ADR-0011](../adr/0011-versionamento-da-api-por-caminho.md) |

Mudanças incompatíveis futuras entram como `/api/v2` sem quebrar consumidores do v1.
O destino do alias `/api` quando o v2 existir **é uma decisão em aberto**.

Estabilidade de contrato é travada por **snapshot** (`insta`, 3 arquivos em
`src/routes/snapshots/`): qualquer mudança de formato exige `cargo insta review`
explícito.

## 3. Cookies

Todos os cookies do sistema, com finalidade e atributos reais:

| Cookie | Finalidade | `HttpOnly` | `SameSite` | `Secure` | `Max-Age` | Sensível |
| --- | --- | :---: | --- | --- | --- | :---: |
| `token` | JWT de acesso | Sim | `Strict` | `COOKIE_SECURE` | `SESSION_TTL_MINUTES` (10 min) | **Sim** |
| `refresh_token` | Refresh token opaco | Sim | `Strict` | `COOKIE_SECURE` | `REFRESH_TTL_DAYS` (14 d) | **Sim** |
| `csrf` | Token anti-CSRF (*double-submit*) | Sim | `Strict` | `COOKIE_SECURE` | — (sessão do navegador) | Sim |
| `flash` | Mensagem de feedback de uso único | Sim | `Strict` | `COOKIE_SECURE` | 1 minuto | Não |
| `lang` | Idioma escolhido explicitamente | — | `Strict` | — | — | Não |

Observações verificáveis:

- **`csrf` não tem `Max-Age`** de propósito: é cookie de sessão do navegador, e não
  rotaciona por página — rotacionar faria duas abas abertas invalidarem uma à outra.
- **`flash` dura 1 minuto**: se nunca for lido, o navegador o descarta. O texto vai
  em **base64** para que acentos sobrevivam ao transporte.
- **Nenhum cookie é legível por JavaScript** exceto `lang`, cujo conteúdo não é
  sensível.

Evidência: `src/auth/session.rs` · `access_cookie`, `refresh_cookie`;
`src/auth/csrf.rs` · `ensure_csrf_token`; `src/routes/flash.rs` · `set_flash`;
`src/i18n.rs` · `lang_cookie`.

## 4. Jobs em segundo plano

Não há fila nem agendador externo. Dois `tokio::spawn` disparados uma vez no boot,
com rodada imediata e depois intervalo fixo:

| Job | Intervalo | Padrão | Desliga com | Escreve no banco? | Concorrência |
| --- | --- | --- | --- | :---: | --- |
| Sincronização de cotações | `QUOTES_SYNC_MINUTES` | 10 min | `0` | **Sim** (`assets`, `portfolio_snapshots`) | `Mutex` |
| Atualização do mercado | `MARKET_SYNC_SECONDS` | 60 s | `0` | **Não** (só memória) | `RwLock` |

Falha de rodada é registrada como `warn` e a próxima tentativa acontece no intervalo
seguinte — cotação atrasada não derruba o serviço.

Detalhe do job de cotações: o `Mutex` fica **adquirido durante a rodada inteira**,
então o botão manual (`POST /quotes/sync`) e o job agendado nunca disparam duas
requisições simultâneas. Chamadas manuais têm cooldown de **30 s** e recebem `429`
dentro dele.

Evidência: `src/app.rs` · `App::start`; `src/quotes.rs` · `spawn_scheduled_sync`,
`QuoteSync::run`, `MANUAL_SYNC_COOLDOWN`; `src/market.rs` ·
`spawn_scheduled_refresh`.

## 5. Convenções válidas para toda resposta

### Cabeçalhos aplicados a **todas** as respostas

Incluindo erros e 404, porque `security_headers` roda antes do roteamento:

| Cabeçalho | Valor | Condição |
| --- | --- | --- |
| `Content-Security-Policy` | `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; form-action 'self'; base-uri 'self'; object-src 'none'` | Sempre |
| `X-Content-Type-Options` | `nosniff` | Sempre |
| `X-Frame-Options` | `DENY` | Sempre |
| `Referrer-Policy` | `no-referrer` | Sempre |
| `Cache-Control` | `no-store` | Sempre, **exceto** `/static/*` |
| `Strict-Transport-Security` | `max-age=63072000; includeSubDomains` | Só se `COOKIE_SECURE=true` |
| `x-request-id` | Id de correlação (propagado ou gerado) | Sempre |

`no-store` fora dos assets é deliberado: telas autenticadas, o CSV e a página de
login (que carrega token CSRF) não devem ficar em cache de navegador, proxy ou
histórico compartilhado.

O `x-request-id` é **sempre devolvido** — o cliente pode citá-lo num reporte de erro
e a linha de log correspondente é encontrada na hora. Id vindo de fora só é aceito se
não vazio, com até 64 caracteres e apenas alfanuméricos ASCII ou `-`.

Verificado por `every_api_response_carries_the_security_headers`, que confere os
cabeçalhos **no sucesso e no erro**.

### Convenções de dado

| Convenção | Regra | Motivo |
| --- | --- | --- |
| **Dinheiro em JSON** | **String**, nunca número (`"unit_value": "10"`) | Um `f64` no meio do caminho anularia a exatidão decimal ([ADR-0004](../adr/0004-decimal-e-numeric-para-dinheiro.md)) |
| Escala monetária | Até 8 casas decimais (`MONEY_SCALE`) | Sub-centavo suficiente para cripto |
| Datas em JSON | Não expostas na API atual | — |
| Datas na interface | Convenção pt-BR | Segue o **dado**, não o idioma da interface |
| Moeda na interface | Sempre BRL (`R$ 1.234,56`) | Idem — a tela em inglês também mostra `R$` |
| CSV | Separador `;`, decimal com vírgula, aspas internas dobradas (RFC 4180) | É o que Excel/LibreOffice em pt-BR leem como coluna |
| `Content-Type` de entrada | `application/json` (API) · `application/x-www-form-urlencoded` (formulários) | — |
| Idioma | Cookie `lang` > `Accept-Language` > pt-BR | — |

### Timeouts, tentativas e idempotência

| Aspecto | Estado real |
| --- | --- |
| Timeout de requisição de entrada | **Não configurado** no servidor |
| Timeout de chamada de saída | **15 s** para ambas as integrações externas |
| Tentativas automáticas de saída | **Nenhuma** — a próxima rodada do job é a retentativa |
| Idempotência de `POST /api/v1/assets` | **Não idempotente** — nome duplicado viola `UNIQUE` |
| Idempotência de `PATCH /api/v1/assets` | **Idempotente** — mesmo corpo, mesmo resultado |
| Idempotência das operações financeiras | **Não idempotentes** — dois `POST /deposit` creditam duas vezes. Não há chave de idempotência |
| Limite de tamanho de corpo | Padrão do axum (2 MB para `Json`/`Form`) |
| *Rate limiting* | **Só no login** (lockout por usuário). Nenhum limite global de requisições |

A ausência de chave de idempotência nas operações financeiras é uma limitação real,
não um esquecimento de documentação: registrada em
[../decisions/known-limitations.md](../decisions/known-limitations.md).

## 6. Especificação OpenAPI

Gerada **do código** por `utoipa` — os `#[utoipa::path]` dos handlers e os
`ToSchema` dos tipos são a única fonte, então a documentação não pode descolar da
implementação.

```bash
curl http://127.0.0.1:3000/api/v1/openapi.json
```

Cobre apenas as três operações de `/api/v1/assets`. **Não** cobre as rotas da
interface HTML nem as sondas — elas não têm consumidor programático.

Dois testes protegem a spec: `openapi_spec_covers_the_asset_routes` (confere que as
rotas e os três verbos estão descritos) e
`the_openapi_spec_is_served_and_describes_the_real_routes` (confere que é JSON válido
com `openapi` e `paths`). O segundo existe porque "spec malformada é pior que
nenhuma: um gerador de cliente a consome sem perguntar."

## 7. Evidências

```text
- src/app.rs             · App::router, security_headers, request_tracing
- src/routes/api.rs      · router, ApiDoc
- src/routes/frontend.rs · router
- src/auth/session.rs    · access_cookie, refresh_cookie
- src/auth/csrf.rs       · ensure_csrf_token
- src/routes/flash.rs    · set_flash
- src/i18n.rs            · lang_cookie, LANG_COOKIE
- src/quotes.rs          · spawn_scheduled_sync, MANUAL_SYNC_COOLDOWN
- src/market.rs          · spawn_scheduled_refresh
- src/routes/snapshots/  (3 snapshots de contrato)
```
