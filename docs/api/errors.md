# Catálogo de erros

## Objetivo

Documentar as 21 variantes de `AppError`, o status HTTP de cada uma, como aparecem
para o cliente e como devem ser interpretadas por quem opera ou consome o sistema.

## Escopo

Coberto: o mapeamento erro → status, a política de censura de 5xx, o formato de
resposta e a diferença entre erro de negócio e erro interno. Não coberto:
procedimentos de diagnóstico (ver [../operations/runbooks.md](../operations/runbooks.md)).

---

## 1. Princípio: 4xx fala, 5xx cala

Toda a política de erro do sistema cabe em cinco linhas, num único lugar:

```rust
let error = if status.is_server_error() {
    tracing::error!(error = ?self, "internal error serving request");
    "internal server error".to_string()
} else {
    self.to_string()
};
```

| Classe | Mensagem ao cliente | Log do servidor |
| --- | --- | --- |
| **4xx** — erro do cliente | A mensagem real | Não registra erro |
| **5xx** — erro nosso | `"internal server error"`, sempre | **Erro completo, com causa raiz** |

Erros 4xx devolvem a mensagem real porque ela é acionável e não revela nada sobre o
funcionamento interno: "saldo insuficiente" ajuda o usuário e não ajuda um atacante.

Erros 5xx são censurados porque a mensagem real conteria texto de erro do SQL, nome
de coluna ou string de conexão. A causa raiz é encadeada automaticamente pelo
`thiserror` (`#[from]` / `#[error(transparent)]`) e vai **inteira** para o log.

**Como investigar um 5xx:** o cliente recebe também o header `x-request-id`. Buscar
esse id no log leva à linha exata, com a causa raiz.

## 2. Formato da resposta

```json
{ "error": "insufficient balance" }
```

Único campo, `error`. Não há código de erro numérico, campo de detalhe, lista de
erros de validação por campo nem `trace_id` no corpo.

> **Limitação de contrato:** um cliente não consegue distinguir programaticamente
> duas causas que compartilham o mesmo status a não ser comparando strings em inglês
> — que não são parte estável do contrato. Registrado em
> [../decisions/known-limitations.md](../decisions/known-limitations.md).

Na **interface HTML**, erros de negócio não aparecem como JSON: viram banner
acessível (`role="alert"`) no formulário de origem, traduzido para o idioma da
sessão, com `autofocus` no primeiro campo.

## 3. Catálogo completo

### 3.1 Erros de autenticação e autorização

| Variante | Status | Mensagem | Quando ocorre |
| --- | :---: | --- | --- |
| `MissingAuthorization` | `400` | `missing authorization header` | Header `Authorization` ausente onde é exigido |
| `InvalidCredentials` | `401` | `invalid credentials` | Senha errada, credencial de admin divergente, sessão sem papel `admin`, username/senha vazios |
| `Jwt(String)` | `401` | `token error: {detalhe}` | Token fabricado, adulterado ou expirado |
| `CsrfMismatch` | `403` | `invalid csrf token` | Token CSRF ausente, vazio ou divergente |
| `TooManyAttempts` | `429` | `too many failed attempts, try again later` | Lockout de login ativo |

> **`MissingAuthorization` é `400`, não `401`.** É uma escolha discutível: a ausência
> do header costuma ser modelada como `401` com `WWW-Authenticate`. O sistema trata
> ausência como requisição malformada e credencial errada como não autorizada.

### 3.2 Erros de registro e identidade

| Variante | Status | Mensagem | Quando ocorre |
| --- | :---: | --- | --- |
| `UsernameTaken` | `400` | `username already taken` | Violação de `UNIQUE` em `users.username`, traduzida |
| `InvalidRegistration` | `400` | `username or password does not meet the registration requirements` | Username fora de 3–32 ou senha fora de 8–128 |
| `UserDoesNotExist` | `404` | `user does not exist` | Usuário não encontrado na autenticação |

> **`UserDoesNotExist` nunca chega ao usuário final na tela de login.** O handler o
> converte na **mesma** mensagem de `InvalidCredentials`, porque mensagens distintas
> vazariam quais contas existem. Travado por
> `business_errors_become_messages_and_internal_errors_do_not`.

### 3.3 Erros de operação financeira

| Variante | Status | Mensagem | Quando ocorre |
| --- | :---: | --- | --- |
| `InvalidAmount` | `400` | `invalid amount` | Quantia ≤ 0, ou escala acima de `MONEY_SCALE` |
| `InsufficientBalance` | `400` | `insufficient balance` | Saldo menor que o total da compra |
| `InsufficientHoldings` | `400` | `insufficient holdings` | Venda maior que a posição |
| `TradeTooSmall` | `400` | `trade total is below the supported monetary precision` | Total arredonda a zero |
| `AssetDoesNotExist` | `404` | `asset does not exist` | `asset_id` inexistente |

**Todos revertem a transação por completo.** Uma compra recusada deixa o saldo
intacto e nenhuma posição criada — travado por
`buy_rejects_when_balance_is_insufficient`.

`TradeTooSmall` existe para impedir um moedor de unidades grátis: sem ele, uma
operação cujo total arredonde a zero moveria unidades sem mover dinheiro.

### 3.4 Erros de catálogo

| Variante | Status | Mensagem | Quando ocorre |
| --- | :---: | --- | --- |
| `InvalidAssetName` | `400` | `asset name must not be empty` | Nome vazio ou só espaços |
| `NegativeUnitValue` | `400` | `unit value must not be negative` | Preço negativo |

Ambos são recusados **antes** de qualquer escrita. Um `500` aqui significaria
validação no banco em vez da borda, e o cliente levaria a culpa pelo próprio erro —
motivo do teste `invalid_payloads_are_rejected_at_the_edge_as_client_errors`.

### 3.5 Erros de integração externa

| Variante | Status | Mensagem | Quando ocorre |
| --- | :---: | --- | --- |
| `QuoteUnavailable` | `502` | `market quote unavailable` | Ativo sem cotação sendo negociado |
| `QuoteSyncTooSoon` | `429` | `quotes were refreshed recently` | Cooldown de 30 s do botão manual |
| `Http(reqwest::Error)` | `502` | *(transparente)* | Falha de rede, timeout, status de erro da fonte |
| `Payload(serde_json::Error)` | `502` | `upstream payload does not match the expected shape: {detalhe}` | Resposta de terceiro fora do formato esperado |

`502` é o status correto para os dois últimos: a falha é **da fonte**, não nossa. A
mensagem do `serde` vai para o log com linha e coluna do corpo — o que torna o
diagnóstico direto quando um campo é renomeado do outro lado.

**`Payload` ser uma variante separada de `Http` é o que permite** decodificar um
payload sem rede, e é isso que os 12 testes de contrato em `tests/payload_*.rs`
exercitam com as respostas reais versionadas.

### 3.6 Erros internos (censurados)

| Variante | Status | Mensagem ao cliente | Quando ocorre |
| --- | :---: | --- | --- |
| `Database(sqlx::Error)` | `500` | `internal server error` | Qualquer falha de SQL não traduzida |
| `Template(askama::Error)` | `500` | `internal server error` | Falha ao renderizar um template |

`Database` é o coletor de tudo que não tem tradução específica. Dois casos concretos
que caem aqui e **deveriam** ser 4xx:

- **Nome de ativo duplicado** (`UNIQUE` em `assets.name`) — ver **DT-06**.
- `value not representable` na leitura de agregado — o incidente de 2026-07-22, hoje
  prevenido por `ROUND` e travado por teste de regressão.

`Template` indica erro de configuração nossa, não do cliente. Em produção é raro:
templates são compilados no binário, então erro de sintaxe já teria quebrado o build.

## 4. Nota técnica sobre `Jwt`

`AppError::Jwt` guarda uma `String`, não o erro original, e **não** usa
`#[from]`/`transparent`. O motivo é registrado no código: `jwt_simple::Error` é um
`anyhow::Error` por baixo e **não implementa `std::error::Error`**, o que impede as
derivações do `thiserror`.

Consequência prática: **a cadeia de causa se perde** nesta variante. É a única do
enum com essa limitação.

## 5. Resumo por status

| Status | Variantes | Significado operacional |
| :---: | --- | --- |
| `400` | 8 | Entrada inválida — corrigir a requisição |
| `401` | 2 | Credencial inválida — autenticar |
| `403` | 1 | CSRF — obter token de um formulário renderizado |
| `404` | 2 | Recurso inexistente |
| `422` | *(do axum)* | Corpo não desserializável, campo obrigatório ausente |
| `429` | 2 | Aguardar — lockout ou cooldown |
| `500` | 2 | **Investigar pelo `x-request-id`** |
| `502` | 4 | Fonte externa indisponível ou fora do formato |
| `503` | *(da sonda)* | `/readyz` com banco inacessível |

## 6. Erros que **não** passam por `AppError`

| Situação | Resposta | Origem |
| --- | --- | --- |
| Rota inexistente | `404` sem corpo JSON | Roteador do axum |
| Método não permitido | `405` | Roteador do axum |
| Corpo malformado ou campo ausente em `Form` | `422` | Extrator do axum |
| Corpo acima do limite | `413` | Extrator do axum |
| Banco inacessível na sonda | `503` sem corpo | `readiness` |

Todos recebem os cabeçalhos de segurança normalmente, porque `security_headers` roda
**antes** do roteamento.

## 7. Como um cliente deve reagir

| Status | Ação recomendada |
| --- | --- |
| `400`, `422` | Não repetir sem corrigir — o erro é determinístico |
| `401` | Reautenticar uma vez; se persistir, parar |
| `403` | Obter um token CSRF novo de um formulário renderizado |
| `404` | Não repetir |
| `429` | **Aguardar.** Lockout: até 15 min. Cooldown de cotação: 30 s |
| `500` | Registrar o `x-request-id` e reportar. Repetir uma vez é aceitável |
| `502` | Repetir com espera; a fonte externa está indisponível |
| `503` | Instância fora de prontidão; tentar outra ou aguardar |

**Não há header `Retry-After`** em nenhuma resposta `429` ou `503` — um cliente
precisa usar os valores documentados acima. Registrado como débito técnico.

## 8. Evidências

```text
- src/error.rs           · AppError (21 variantes), IntoResponse, From<jwt_simple::Error>
- src/routes/flash.rs    · business_flash (quais erros viram banner)
- src/repository.rs      · validated_asset_name, validated_unit_value,
                           deposit, buy_asset, sell_asset
- src/auth/user.rs       · register (tradução de UNIQUE para UsernameTaken)
- src/quotes.rs          · QuoteSync::run (QuoteSyncTooSoon)
- src/app.rs             · readiness (503)
- testes: invalid_payloads_are_rejected_at_the_edge_as_client_errors,
          malformed_json_bodies_never_reach_the_handler,
          patching_an_unknown_asset_is_a_404,
          a_business_error_comes_back_as_a_banner_not_a_500,
          business_errors_become_messages_and_internal_errors_do_not,
          a_malformed_payload_becomes_a_typed_error_not_a_panic
```
