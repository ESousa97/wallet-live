# Roadmap

Evolução planejada do **wallet**. A ordem reflete as prioridades do projeto:
**correção financeira → segurança → arquitetura → operação → produto**. Um item
só avança de fase quando o anterior tem rede de testes cobrindo o comportamento.

## ✅ Fase 1 — Fundações e integridade de dados

- [x] Valores monetários exatos de ponta a ponta (`rust_decimal` ↔ `NUMERIC`);
  nada de ponto flutuante em dinheiro.
- [x] Livro-razão transacional: posições materializadas (`holdings`) + extrato
  imutável (`transactions`), com depósito/compra/venda atômicos (`FOR UPDATE`,
  rollback íntegro) e custo médio ponderado.
- [x] Sessão stateless: JWT assinado (HS256) em cookie `HttpOnly` +
  `SameSite=Strict` + `Secure` configurável, com expiração do cookie alinhada à
  validade do token.
- [x] Configuração *fail-fast*: segredos lidos e validados uma única vez no boot;
  serviço não sobe com configuração incompleta.
- [x] Erros 5xx censurados na resposta (detalhes só no log estruturado do servidor).
- [x] Comparação em tempo constante da credencial administrativa.
- [x] Guard-rails no banco: `CHECK` de preço/quantidade/saldo não negativos e
  tipos de transação restritos — o schema é a última linha de defesa.
- [x] Validação de entrada na borda da API (nome de ativo não vazio, preço não
  negativo, quantias positivas nas operações).
- [x] Sonda `/health` (serviço + banco), desligamento gracioso e `tracing` com
  nível configurável.
- [x] Testes do núcleo financeiro em bancos efêmeros isolados (`#[sqlx::test]`)
  e snapshots do contrato JSON da API (`insta`).

## ✅ Fase 2 — Segurança de sessão

- [x] Proteção CSRF nos formulários (*double-submit cookie* com comparação em
  tempo constante, somada ao `SameSite=Strict`).
- [x] *Refresh token* opaco com **rotação a cada uso** e revogação no servidor
  (tabela `sessions` guarda apenas o hash SHA-256); o access token continua um
  JWT curto, renovado de forma transparente por middleware.
- [x] Logout com revogação real da sessão (não apenas remoção do cookie).
- [x] *Rate limiting* no login: lockout progressivo por usuário com backoff
  exponencial (429), zerado no login correto.
- [x] Papéis de usuário (`role`): sessões de admin autorizam a API
  administrativa; a secret key permanece como credencial de serviço
  máquina-a-máquina.
- [x] Cabeçalhos de segurança: CSP, `X-Content-Type-Options`, `X-Frame-Options`,
  `Referrer-Policy` e HSTS quando atrás de HTTPS.
- [x] TTLs de sessão configuráveis por ambiente (`SESSION_TTL_MINUTES`,
  `REFRESH_TTL_DAYS`).
- [x] Ativos estáticos servidos localmente: o bundle do Tailwind é embutido no
  binário e servido em `/static/tailwind.js` — zero requisições a terceiros e
  `script-src 'self'` na CSP.

## 🏗️ Fase 3 — Arquitetura e API

- [x] Paginação real do extrato (`/assets?page=N`, ordem estável, navegação na
  interface).
- [x] Versionamento da API: caminho canônico `/api/v1` (com `/api` mantido como
  alias de compatibilidade).
- [x] Camada de serviço explícita: `PortfolioService` orquestra a visão da
  carteira (consultas concorrentes + paginação) e as operações; handlers só
  fazem HTTP, o repository só fala SQL.
- [x] Especificação OpenAPI gerada do código (utoipa), servida em
  `/api/v1/openapi.json` — a documentação não descola da implementação.
- [x] Avaliação de UUIDv7 como chave pública. **Decisão: manter `BIGSERIAL`.**
  Os ids só aparecem em superfícies autenticadas (API administrativa e
  formulários da própria sessão) e toda leitura é filtrada pelo usuário da
  sessão — enumeração não expõe dados de terceiros. Reavaliar se a API se
  tornar pública ou multi-tenant.
- [x] Abstração do repositório via trait: `PortfolioRepository` cobre o
  subconjunto que o `PortfolioService` usa (`Repository` o implementa
  delegando aos métodos inerentes); testes de orquestração — montagem da
  `WalletView`, propagação de erro de depósito/compra/venda — rodam contra um
  dublê em memória, sem Postgres.

## ⚙️ Fase 4 — Operação

- [x] CI (GitHub Actions): `fmt --check`, `clippy -D warnings`, testes com
  Postgres em service container, `cargo sqlx prepare --check` (o cache offline
  `.sqlx/` é versionado — lint e build dispensam banco), `cargo audit` e build
  da imagem Docker.
- [x] Dockerfile multi-stage: binário único (templates e migrações embutidos),
  usuário sem privilégios, `SQLX_OFFLINE`; serviço `app` no compose (perfil
  opcional) com healthchecks; suporte a CA extra para ambientes com inspeção
  TLS (proxy corporativo/antivírus).
- [x] Migrações aplicadas automaticamente no boot — o serviço nunca sobe com
  schema defasado e o deploy dispensa passo manual.
- [x] Logs em JSON (`LOG_FORMAT=json`) e `request_id` por requisição
  (propagado do `x-request-id` quando bem-formado, senão gerado; devolvido na
  resposta e presente em todos os logs do span).
- [x] Sondas separadas: liveness (`/healthz`, sem tocar o banco) e readiness
  (`/readyz`, exige o banco são).
- [x] Exportação OpenTelemetry: cada requisição vira um trace (span `request`
  com os `#[instrument]` dos handlers aninhados dentro) e alimenta o
  histograma `http.server.request.duration`, exportados via OTLP/HTTP quando
  `OTEL_EXPORTER_OTLP_ENDPOINT` está definida — ausente, zero overhead, zero
  tentativa de conexão. Coletor local (`docker compose --profile observability
  up otel-collector`) para verificar a exportação sem montar um backend de
  verdade.

## 📈 Fase 5 — Produto e UX

- [x] Mensagens de feedback nos formulários: erros de negócio e confirmações
  viram banners acessíveis (`role=alert/status`) em pt-BR via *flash cookie*
  de uso único; a tela de login não confirma mais se um username existe.
- [x] Exportação do extrato em CSV (`/transactions.csv`, autenticado, convenção
  pt-BR de planilha).
- [x] Atualização agendada de cotações: job em segundo plano com rodada no boot
  e intervalo configurável (`QUOTES_SYNC_MINUTES`, zero desliga).
- [x] Formatação pt-BR consistente (moeda, quantidades, datas, CSV).
- [x] Mais pares de mercado na sincronização de cotações: uma única chamada
  BRL→\* cobre USD, EUR, BTC, ETH e SOL (preço = inverso da taxa).
- [x] Gráfico de evolução do patrimônio: snapshots por rodada de cotações,
  desenhados como SVG server-side (zero JavaScript, amigável à CSP).
- [x] Acessibilidade: erro de negócio volta ao formulário de origem com
  `autofocus` no primeiro campo, `aria-label` nos campos sem rótulo visível,
  anel de foco uniforme (`:focus-visible`) e *skip link* para o conteúdo.
- [x] Operações sem recarregar a página, mantendo SSR: htmx (vendorado no
  binário, CSP intacta) troca só o fragmento da carteira; requisições `HX-Request`
  recebem o parcial com flash inline e `HX-Push-Url` numa resposta única, sem
  JavaScript o fluxo clássico de redirect continua inteiro (*progressive
  enhancement*).
- [x] Internacionalização dos templates: pt-BR e inglês via catálogo tipado
  (`i18n::Strings` — texto faltando num idioma é erro de compilação, campo
  inexistente no template também). Idioma por cookie (`/lang/{code}`, com
  `?next=` validado contra open redirect) > `Accept-Language` > pt-BR; flashes
  de sucesso e de erro de negócio acompanham. Moeda, datas e CSV ficam na
  convenção do dado (BRL/planilha pt-BR), não da interface.
