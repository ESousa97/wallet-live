# ADR-0002: axum em vez de Rocket

## Status

Aceita.

## Contexto

O bootcamp ensinou **Rocket** como framework web principal (módulo M17), mas o
próprio módulo de Projeto Final usou axum. O projeto precisava escolher entre
seguir o framework das aulas teóricas ou o do projeto de referência.

O requisito que decidiu a questão é específico deste domínio: o sistema tem rotas
com **três níveis distintos de acesso** — públicas (`/login`, `/healthz`, assets),
autenticadas (`/assets`, `/market`, operações) e administrativas
(`POST`/`PATCH /api/v1/assets`). Um erro de configuração que deixasse uma rota
financeira sem autenticação seria uma falha grave e silenciosa: a rota funcionaria,
apenas sem exigir sessão.

## Restrições

- Rust (ver [ADR-0001](0001-rust-como-linguagem-unica.md)).
- Necessidade de SSR com templates, não só JSON.
- A suíte de testes precisa exercitar os middlewares na ordem real, sem abrir
  socket nem porta (requisito de CI e de execução local em Windows).
- Autor único: a proteção de rota precisa ser **evidente na leitura**, não
  dependente de disciplina de configuração.

## Opções consideradas

**Avaliadas de fato**, com comparação documentada em
[../aprendizado/09-frameworks-web-rocket-vs-axum.md](../aprendizado/09-frameworks-web-rocket-vs-axum.md):

1. **Rocket** — framework das aulas. Macros de rota (`#[get("/path")]`),
   ergonomia inicial melhor, autenticação tipicamente por *fairing* (middleware
   global).
2. **axum** — usado no módulo de Projeto Final. Extratores como parâmetros de
   handler, construído sobre `tower`.

**Comparação *post hoc***: actix-web, warp, hyper puro.

## Decisão

axum 0.8.9, com `axum-extra` para cookies.

## Fundamentação

**Motivo confirmado**, e o argumento central é o **modelo de extratores**.

Em axum, o handler declara nos parâmetros o que exige:

```rust
async fn buy_asset(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,   // exige sessão válida
    portfolio: PortfolioService,
    ...
)
```

O axum resolve cada extrator chamando `FromRequestParts::from_request_parts`
**antes** de o corpo do handler rodar. Se um extrator falha, o handler **nunca
executa**. Três consequências que o modelo de middleware global do Rocket não
oferece:

1. **A proteção é uma propriedade da assinatura**, visível na mesma tela onde o
   handler é lido — não numa lista de exceções em outro arquivo, que alguém
   esquece de atualizar ao adicionar rota.
2. **Autorização como tipo.** `Admin` não é um booleano verificado no corpo do
   handler; é um parâmetro cuja construção *é* a verificação. Não existe caminho em
   que o handler rode sem ela.
3. **Composição com `tower`.** É isto que permite à suíte empurrar requisições pelo
   router **de produção** com `tower::oneshot`, sem socket. O comentário em
   `App::router` é explícito sobre o motivo: um teste que montasse o seu próprio
   router "provaria apenas que o handler funciona — e deixaria de fora a CSP, a
   renovação de sessão e o span da requisição, que são justamente as camadas que
   ninguém lembra de conferir à mão."

## Consequências positivas

- Rota desprotegida é visível na assinatura; 8 testes em `tests/http_api.rs` e 15
  em `tests/http_web.rs` exercitam a pilha real por causa da composição `tower`.
- Um extrator novo (`Repository`, `PortfolioService`, `Locale`, `SessionUser`,
  `HxRequest`) serve como injeção de dependência sem framework de DI.
- Middlewares são funções comuns (`from_fn_with_state`), testáveis e legíveis.

## Consequências negativas

- **Mensagens de erro hostis.** Um handler cuja assinatura não satisfaz os traits
  produz erro longo e indireto, tipicamente apontando para o `.route()` em vez do
  handler.
- **Ordem de `.layer()` invertida** em relação à leitura: a última adicionada
  executa primeiro. Exigiu comentário explicativo no código — e o comentário
  existe, porque sem ele a ordem parece arbitrária.
- **Rupturas em versão menor.** A migração 0.7 → 0.8 mudou a sintaxe de parâmetro
  de rota (`:code` → `{code}`).
- Menos ergonomia inicial que as macros de rota do Rocket.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Quebra de API em versão menor do axum | Médio — refactor pontual a cada atualização | Versão fixada em `Cargo.toml`; `Cargo.lock` versionado |
| Ordem de camadas alterada por engano | **Alto** — `refresh_session` antes de `security_headers` quebraria a renovação de sessão ou a CSP | Comentário explicando a ordem; testes que verificam cabeçalhos em resposta de erro |
| Extrator novo esquecer verificação | Alto | O padrão é que a construção seja a verificação; revisão de código |

## Evidências

```text
- Cargo.toml                  (axum 0.8.9, axum-extra 0.10.1)
- src/app.rs                  · App::router (ordem das camadas comentada)
- src/auth/admin.rs           · impl FromRequestParts for Admin
- src/auth/user.rs            · impl FromRequestParts for User e Option<User>
- src/routes/frontend.rs      · SessionUser, HxRequest
- tests/common/mod.rs         (tower::oneshot sobre App::router)
- docs/aprendizado/09-frameworks-web-rocket-vs-axum.md
```

## Critérios de revisão

Reavaliar se:

1. O axum publicar 1.0 com quebras que exijam refactor extenso — momento natural
   para comparar novamente.
2. O projeto precisar de WebSocket ou streaming em escala que exponha limitação da
   pilha `tower`.
3. A frequência de quebras em versão menor passar a custar mais do que o modelo de
   extratores rende.
