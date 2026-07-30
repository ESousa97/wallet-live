# wallet

Carteira digital de investimentos com **backend e renderização SSR em Rust**,
servidos pelo mesmo binário. O htmx, mantido localmente, adiciona melhorias
progressivas à interface. A aplicação reúne API REST administrativa (JSON),
**valores monetários exatos**, operações **transacionais** e cotações de mercado
reais.

> Avaliação rápida: [ENTREGA.md](docs/ENTREGA.md) reúne a matriz de requisitos do
> bootcamp, as decisões de escopo e um roteiro curto de demonstração.
> Detalhe técnico: [docs/ARQUITETURA.md](docs/ARQUITETURA.md) explica, etapa
> por etapa e a partir do código, como o sistema é montado e por que cada
> decisão foi tomada.

## Funcionalidades

- **Carteira completa** — saldo em caixa, depósito, compra e venda de ativos ao
  preço de mercado, custo médio ponderado por posição, lucro/prejuízo por ativo
  e resumo do patrimônio.
- **Extrato** — livro-razão imutável de transações (depósitos, compras,
  vendas), paginado na interface e exportável em **CSV**.
- **Cotações reais** — preços (`USD→BRL`, `BTC→BRL`) da API pública da
  Coinbase, com **sincronização agendada** em segundo plano (e botão manual),
  criação automática do catálogo inicial numa instalação vazia e atualização
  em um único `UPDATE` (sem N+1). Chamadas manuais são serializadas e têm
  cooldown.
- **Painel de mercado** — dashboard da moeda escolhida (cotação, variações de
  1 h/24 h/7 d, capitalização, volume, faixa de negociação do dia, máxima
  histórica e oferta em circulação) com **gráfico temporal** em 24 h ou 7 d, ao
  lado de um cartão fixo com as 100 maiores criptomoedas, rolagem própria e
  busca por nome ou ticker. Tudo vem de um snapshot em memória: trocar de moeda
  ou de janela não custa nenhuma chamada externa. A direção nunca é comunicada
  só por cor — toda variação sai com seta ▲/▼ **e** sinal.
- **Feedback nas operações** — sucessos e erros de negócio viram banners
  acessíveis em pt-BR (flash messages), nunca JSON cru na tela.
- **Operações sem recarregar a página** — os formulários e a navegação da
  carteira trocam só o fragmento HTML do miolo via **htmx** (servido do próprio
  binário); sem JavaScript tudo continua funcionando pelo fluxo clássico de
  redirect (*progressive enhancement*), e o servidor segue dono do HTML (SSR).
- **Interface multi-idioma** — pt-BR e inglês, escolhidos pelo seletor da
  interface (cookie) ou pelo `Accept-Language` do navegador; mensagens de
  feedback acompanham o idioma. Moeda e datas ficam na convenção do dado (BRL),
  não da interface.
- **Autenticação e sessão** — cadastro/login com hash de senha (argon2); sessão
  com **JWT de acesso curto** + **refresh token rotacionado e revogável**
  (logout mata a sessão no servidor), ambos em cookies `HttpOnly` +
  `SameSite=Strict`; **lockout progressivo** contra força bruta e **CSRF
  tokens** em todos os formulários.
- **Privacidade no navegador** — páginas dinâmicas, autenticação e CSV usam
  `Cache-Control: no-store`; apenas CSS e htmx podem ser armazenados em cache.
- **API administrativa** — catálogo de ativos sob `/api/v1`, autorizada por
  **papel de usuário** (sessão de admin) ou por credencial de serviço com
  comparação em tempo constante.
- **Pronto para orquestração** — migrações aplicadas no boot, sondas de
  liveness/readiness separadas (`/healthz`, `/readyz`), desligamento gracioso,
  logs estruturados com `request_id` por requisição (JSON opcional), traces e
  métricas exportáveis via OTLP e imagem Docker multi-stage.

## Decisões de engenharia

| Tema | Decisão |
| --- | --- |
| Dinheiro | `rust_decimal::Decimal` ↔ `NUMERIC` no Postgres. Ponto flutuante nunca entra nos cálculos nem na persistência do núcleo financeiro; `f64` aparece apenas nas coordenadas visuais do gráfico SVG. Escala canônica de **8 casas** em toda gravação (`MONEY_SCALE`) e `ROUND` nos agregados do SQL: `NUMERIC` é ilimitado, `Decimal` tem 28 dígitos significativos — sem o invariante, um preço vindo de `1/taxa` torna somas e produtos indecodificáveis na leitura. |
| Consistência | Compra/venda/depósito rodam em transação com `FOR UPDATE`; saldo insuficiente reverte tudo. O schema tem `CHECK`s (saldo, preço e quantidade não negativos) como última linha de defesa. |
| Modelo de dados | `holdings` materializa a posição atual por (usuário, ativo); `transactions` é o histórico imutável. Leituras triviais, escrita explícita. |
| SQL | As queries principais usam `sqlx::query!`/`query_as!` e são **checadas em tempo de compilação** contra o banco. Consultas dinâmicas pequenas ficam restritas ao bootstrap do catálogo e ao health check. |
| Injeção de dependência | Extratores do Axum (`Repository`, `User`, `Admin`): a assinatura do handler declara o que ele exige; sem satisfazer, o handler nem roda. |
| Sessão | JWT de acesso curto (stateless) + refresh token opaco com rotação a cada uso e hash SHA-256 no banco — revogável de verdade, replay de token queimado não funciona. Renovação transparente via middleware. |
| Defesas HTTP | CSRF *double-submit* nos formulários, lockout com backoff no login, CSP + `nosniff` + `X-Frame-Options` + `Referrer-Policy` em toda resposta, HSTS atrás de HTTPS. |
| Erros | Enum único (`AppError`) mapeado para status HTTP corretos; falhas 5xx são logadas com causa raiz e respondidas com mensagem genérica (nada de detalhe interno na resposta). |
| Configuração | Lida e validada **uma vez** no boot (*fail-fast*): segredo ausente derruba o serviço com mensagem clara, não um 401 confuso em produção. |
| Templates | Askama — variáveis dos templates também checadas em compilação. |
| Interatividade | htmx com HTML parcial: a mesma visão da carteira renderiza a página inteira (`assets.html`) ou só o fragmento (`wallet.html`) conforme o header `HX-Request`; operações respondem o fragmento atualizado na própria resposta (uma requisição, flash inline, `HX-Push-Url`). Sem o header (sem JS, restauração de histórico), vale o PRG clássico. |
| i18n | Catálogo tipado (`i18n::Strings`, uma `const` por idioma): texto faltando é erro de compilação, e o askama checa os campos usados nos templates. Resolução: cookie `lang` > `Accept-Language` > pt-BR. |
| CSS | Compilado em build-time pelo CLI standalone do Tailwind (executável único — **sem Node e sem npm**, então o build não herda a cadeia de suprimentos do ecossistema JS) e versionado como o cache `.sqlx`, com o CI conferindo o frescor. O `source(none)` desliga a varredura automática: sem ele o gerador lê o próprio output e o build deixa de ser determinístico entre plataformas. |
| Mercado | Fonte: API pública da CoinGecko (sem chave, já em BRL, 100 moedas por chamada, variação de 24 h pronta). O snapshot vive **em memória**, não no banco: é dado de terceiro e volátil, perder no restart não custa nada, e gravar misturaria cotação informativa com o catálogo que lastreia as operações. Um job atualiza a cada `MARKET_SYNC_SECONDS`; a requisição do usuário só lê o snapshot, então a tela responde igual com um ou mil acessos e o limite da API não depende do tráfego. **Este feed não move dinheiro** — o preço de compra e venda continua vindo de `assets.unit_value`, gravado de taxas que chegam como string e viram `Decimal` sem passar por float; a CoinGecko devolve número JSON (f64), precisão boa para exibir e ruim para contabilizar. |
| Cor | Paleta validada por script (banda de luminosidade, croma, separação sob daltonismo, contraste) contra a superfície real. Verde↔vermelho medem ΔE ~4,6 sob deuteranopia, então **nenhuma variação é comunicada só por cor** — sempre com seta ▲/▼ e sinal. O acento violeta não disputa hue com o par lucro/prejuízo. |

## Estrutura

```
src/
  main.rs            # enxuta: tokio::main -> App::start()
  lib.rs             # os módulos como BIBLIOTECA — é o que permite a `tests/`
                     # existir (teste de integração não importa de um binário)
  app.rs             # boot, AppState { db, config, ... }, /health, shutdown gracioso,
                     # tracing + métricas (exportação OTLP opcional)
  config.rs          # Config: lê e valida o ambiente uma vez (fail-fast)
  i18n.rs            # idiomas da interface (pt-BR/en): catálogo tipado + extrator
  models.rs          # Asset, UserRecord, WalletSummary, Holding, Transaction
  error.rs           # AppError + IntoResponse (status HTTP, censura de 5xx)
  quotes.rs          # cotações de mercado (Coinbase) -> preços dos ativos
  market.rs          # snapshot informativo das 100 maiores moedas (CoinGecko),
                     # série temporal projetada e medidor da faixa do dia
  repository.rs      # todo o acesso ao banco (queries + transações) + testes
  auth/
    admin.rs         # extrator Admin (sessão com role admin OU credencial de serviço)
    user.rs          # User/UnauthenticatedUser, hash de senha, JWT, extratores
    session.rs       # refresh token (rotação/revogação) + middleware de renovação
    csrf.rs          # proteção CSRF (double-submit cookie)
    throttle.rs      # lockout progressivo de login
  services/
    portfolio.rs     # PortfolioService: visão da carteira + operações, genérico sobre
                      # o trait PortfolioRepository (testável sem banco)
  routes/
    api.rs           # API REST administrativa (JSON) + OpenAPI + testes de snapshot
    frontend.rs      # SSR: login/logout, carteira, operações, filtros Askama
tests/               # suíte de CONTRATO (a de unidade vive ao lado do código)
  payloads/          # respostas REAIS da CoinGecko e da Coinbase, versionadas
  payload_market.rs  # o payload da CoinGecko atravessa o parse de produção
  payload_quotes.rs  # idem para as taxas de câmbio (strings de 41 dígitos)
  http_api.rs        # requisições HTTP pelo router real: autorização, alias, spec
  http_web.rs        # sessão, CSRF, fluxo de dinheiro e mercado pelo navegador
  common/mod.rs      # andaime: monta o app de produção sem abrir socket
  fixtures/          # estado inicial de banco para as baterias HTTP
templates/           # base.html + login.html + assets.html + wallet.html
                     # + market.html + market_dashboard.html (fragmentos htmx)
                     # + market_macros.html (blocos repetidos do painel)
migrations/          # schema versionado, up/down reversíveis
docs/                # ENTREGA.md, ROADMAP.md, TESTES.md (o que cada teste trava
                     # e por que existe), ARQUITETURA.md (o sistema explicado a
                     # partir do código) e docs/aprendizado/
                     # (aula do curso -> decisão -> motivo)
```

## Rotas

### Interface do usuário (HTML, na raiz)

| Método | Rota | Auth | Descrição |
| --- | --- | --- | --- |
| `GET` | `/healthz` | — | Liveness: o processo responde (não depende do banco) |
| `GET` | `/readyz` · `/health` | — | Readiness: pronto para tráfego (banco respondendo) |
| `GET` | `/login` · `/register` | — | Formulários de login / cadastro |
| `POST` | `/login` · `/register` | — | Autentica ou cadastra; grava o cookie de sessão |
| `GET` | `/logout` | — | Revoga a sessão no servidor e remove os cookies |
| `GET` | `/` | opcional | Com sessão vai para `/assets`; sem, para `/login` |
| `GET` | `/assets` | sessão | Carteira: saldo, posições, resumo e extrato (paginado via `?page=`) |
| `GET` | `/market` | sessão | Painel de mercado: `?coin=` escolhe a moeda, `?range=24h\|7d` a janela do gráfico, `?q=` busca na lista |
| `GET` | `/transactions.csv` | sessão | Download do extrato completo em CSV |
| `GET` | `/static/app.css` · `/static/htmx.js` · `/static/money-input.js` | — | Assets servidos do próprio binário (sem CDN) |
| `GET` | `/deposit` · `/buy` · `/sell` | sessão | Carteira com o formulário da operação aberto |
| `GET` | `/lang/{code}` | — | Troca o idioma da interface (`pt-BR`/`en`) e volta para `?next=` |
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
| `LOG_FORMAT` | não (texto) | `json` emite uma linha JSON por evento (agregadores de log) |
| `QUOTES_SYNC_MINUTES` | não (`10`) | Intervalo do job de cotações (`0` desliga) |
| `MARKET_SYNC_SECONDS` | não (`60`) | Intervalo do job da tela de mercado (`0` desliga) |
| `RUST_LOG` | não (`info`) | Nível de log (ex.: `wallet=debug,info`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | não (desligado) | Endpoint OTLP (HTTP) para exportar traces e métricas — ausente, nada é exportado |
| `OTEL_SERVICE_NAME` | não (`wallet`) | Nome do serviço no backend de observabilidade |

## Como rodar

Pré-requisitos: [Rust](https://rustup.rs) e Docker (ou um Postgres próprio).

```powershell
# 1) subir o Postgres
docker compose up -d

# 2) configurar o ambiente (copie o exemplo e ajuste os segredos)
Copy-Item .env.example .env

# 3) rodar — as migrações são aplicadas automaticamente no boot
cargo run
```

> O cache `.sqlx/` versionado permite compilar sem banco (`SQLX_OFFLINE=true`).
> Para o fluxo de desenvolvimento com banco vivo, a CLI ajuda a criar novas
> migrações e a regenerar o cache:
> `cargo install sqlx-cli --no-default-features --features postgres,rustls`
> e depois `cargo sqlx migrate add -r <nome>` / `cargo sqlx prepare`.

### Stack completo em Docker

```powershell
docker compose --profile app up --build
```

Builda a imagem de produção (multi-stage, binário único com templates e
migrações embutidos) e sobe app + banco com healthchecks. Em máquinas atrás de
proxy corporativo ou antivírus com inspeção TLS, veja
`docker/extra-ca/README.md`.

Abra <http://localhost:3000>, cadastre um usuário e use a carteira: deposite,
compre/venda ativos e visite o mercado. A primeira sincronização bem-sucedida
cria automaticamente o catálogo de USD, EUR, BTC, ETH e SOL com preços reais;
se a rede estiver indisponível, o estado vazio oferece o botão de tentar
novamente e a API administrativa abaixo permite cadastrar um ativo manual.
A sessão persiste em cookies seguros. Um access token expirado é renovado
silenciosamente enquanto o refresh token ainda é válido; somente uma sessão
ausente, revogada ou totalmente expirada volta a página inteira para o login,
inclusive em requisições htmx.

### Observabilidade (traces e métricas)

Com `OTEL_EXPORTER_OTLP_ENDPOINT` definida, cada requisição HTTP vira um trace
(span `request`, com os spans dos handlers `#[instrument]` aninhados dentro) e
alimenta o histograma `http.server.request.duration` (rotulado por método,
rota e status) — exportados via OTLP/HTTP para o endpoint configurado. Sem a
variável, não há exportação nem tentativa de conexão externa; os instrumentos
locais continuam funcionando como operações sem efeito.

Para ver a exportação funcionando localmente, sem montar um backend de
observabilidade de verdade:

```powershell
docker compose --profile observability up -d otel-collector
$env:OTEL_EXPORTER_OTLP_ENDPOINT = 'http://localhost:4318'
cargo run
```

`docker compose logs -f otel-collector` mostra cada trace e cada ponto de
métrica recebido — o coletor só imprime, não repassa a lugar nenhum
(`docker/otel-collector/config.yaml`).

### Exemplo de uso da API administrativa

```powershell
$admin = @{ Authorization = $env:ADMIN_SECRET_KEY }

Invoke-RestMethod http://127.0.0.1:3000/api/v1/assets

$asset = Invoke-RestMethod -Method Post http://127.0.0.1:3000/api/v1/assets -Headers $admin `
  -ContentType 'application/json' -Body '{"name":"ouro","unit_value":750.25}'

Invoke-RestMethod -Method Patch http://127.0.0.1:3000/api/v1/assets -Headers $admin `
  -ContentType 'application/json' `
  -Body (@{ id = $asset.id; unit_value = 760.10 } | ConvertTo-Json)
```

## Testes

**118 testes em duas camadas.** O catálogo completo — o que cada um trava e por
que ele existe — está em [TESTES.md](docs/TESTES.md).

```powershell
docker compose up -d db
cargo test
```

Só o que não toca banco (rápido, roda em qualquer máquina):

```powershell
cargo test --test payload_market --test payload_quotes
```

- **Unidade** (83), ao lado do código, com acesso ao que é privado: projeção de
  gráfico no `viewBox`, inversão de taxa, montagem de URL, e o **núcleo
  financeiro** contra Postgres real — depósito, compra, venda, custo médio
  ponderado, guardas de saldo/posição e validação de entrada.
- **Contrato** (35), em `tests/`, atravessando as mesmas funções públicas que o
  servidor atravessa. Existe porque o crate tem um alvo de biblioteca
  (`src/lib.rs`): teste de integração é um crate separado e não importa de um
  binário.
- Os **payloads das integrações externas são reais**, capturados do endpoint de
  produção e versionados em [`tests/payloads/`](tests/payloads/README.md). Não é
  preciosismo: a Coinbase entrega as taxas como string com precisão arbitrária
  (a maior da captura tem **41 dígitos significativos**, contra os 28 da mantissa
  do `Decimal`) e decodifica as 636 de uma vez — uma taxa que não caiba derruba a
  sincronização de todos os pares. Um fixture com `"BTC": 0.0000031` passaria
  para sempre sem nunca tocar nisso.
- As baterias HTTP montam o **router de produção** (`App::router`) e empurram
  requisições por ele com `tower::oneshot` — sem socket, sem porta. Assim os
  middlewares (CSP, renovação de sessão, span da requisição) são exercitados na
  ordem real, e não contornados.
- `#[sqlx::test]` cria um **banco efêmero por teste** (migrações aplicadas
  automaticamente), então os testes são isolados e paralelos.
- O contrato JSON da API é congelado com **insta** (snapshot testing):
  `cargo insta review` para auditar mudanças de formato.

## Roadmap

O plano de evolução — segurança de sessão, camada de serviço, CI/CD,
observabilidade e novas funcionalidades — está em [ROADMAP.md](docs/ROADMAP.md).

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
