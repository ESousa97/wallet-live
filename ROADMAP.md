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
- [ ] Ativos estáticos servidos localmente (sem CDN de terceiros).

## 🏗️ Fase 3 — Arquitetura e API

- [x] Paginação real do extrato (`/assets?page=N`, ordem estável, navegação na
  interface).
- [x] Versionamento da API: caminho canônico `/api/v1` (com `/api` mantido como
  alias de compatibilidade).
- [ ] Camada de serviço explícita (ex.: `PortfolioService`) separando regra de
  negócio dos handlers HTTP.
- [ ] Abstração do repositório via trait, permitindo testes de serviço sem banco.
- [ ] Especificação OpenAPI gerada do código.
- [ ] Avaliação de UUIDv7 como chave pública dos recursos (evita enumeração).

## ⚙️ Fase 4 — Operação

- [ ] CI: `fmt --check`, `clippy -D warnings`, testes com Postgres em service
  container, `sqlx prepare --check` (build offline) e auditoria de dependências.
- [ ] Dockerfile multi-stage do serviço + compose completo (app + banco).
- [ ] Migrações aplicadas automaticamente no boot.
- [ ] Logs em JSON, propagação de `request_id` e exportação OpenTelemetry.
- [ ] Sondas de *liveness*/*readiness* separadas.

## 📈 Fase 5 — Produto e UX

- [ ] Operações sem recarregar a página (HTML parcial/htmx), mantendo SSR.
- [ ] Mensagens de feedback (sucesso/erro) nos formulários.
- [ ] Formatação por locale e internacionalização dos templates.
- [ ] Acessibilidade: foco gerenciado nos formulários inline, navegação por teclado.
- [ ] Atualização agendada de cotações (job periódico) e mais pares de mercado.
- [ ] Exportação do extrato (CSV) e gráfico de evolução do patrimônio.
