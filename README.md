# wallet-live

Carteira digital de investimentos com **backend, regras financeiras e HTML renderizado
no servidor escritos em Rust**, servidos por um único binário.

> **Simulação educacional.** Não movimenta dinheiro real, não integra meio de pagamento
> e não oferece recomendação de investimento. Projeto final do bootcamp Santander 2026 —
> Rust AI Developer (DIO).

**[📖 Documentação técnica completa](docs/README.md)**

---

## Problema que resolve

Uma carteira de investimentos precisa registrar aportes, executar compras e vendas a
preço de mercado, calcular custo médio e resultado por posição, e manter um histórico
auditável — sem jamais perder precisão monetária.

O material didático que originou o projeto modelava dinheiro como ponto flutuante e o
histórico como um log de compras que só sabia somar. Nenhum dos dois sobrevive a um
produto que também **vende**: ponto flutuante acumula erro
([ADR-0004](docs/adr/0004-decimal-e-numeric-para-dinheiro.md)) e um log append-only não
representa saída ([ADR-0005](docs/adr/0005-holdings-materializados-e-livro-razao.md)).

## Escopo

**Faz:** aporte de caixa, compra e venda de ativos ao preço de mercado, custo médio
ponderado, resultado por posição, extrato imutável, painel de mercado informativo, API
administrativa de catálogo.

**Não faz:** saque, transferência entre usuários, ordem limitada ou agendada, estorno,
múltiplas moedas de denominação, recuperação de senha. Lista completa em
[known-limitations.md](docs/decisions/known-limitations.md).

## Funcionalidades

- **Carteira** — saldo em caixa, depósito, compra e venda ao preço de mercado, custo
  médio ponderado por posição, resultado por ativo e resumo do patrimônio.
- **Extrato** — livro-razão imutável, paginado na interface e exportável em CSV.
- **Cotações reais** — preços da API pública da Coinbase, com sincronização agendada,
  criação automática do catálogo mínimo numa instalação vazia e atualização num único
  `UPDATE`.
- **Painel de mercado** — as 100 maiores criptomoedas em BRL, com gráfico temporal em
  24 h ou 7 d, servido de um snapshot em memória: trocar de moeda não custa chamada
  externa.
- **Operações sem recarregar a página** — htmx troca só o fragmento da carteira; sem
  JavaScript, o fluxo clássico de redirect continua inteiro.
- **Interface bilíngue** — pt-BR e inglês, a partir de um catálogo tipado.
- **Autenticação e sessão** — argon2, JWT de acesso curto + refresh token rotacionado e
  revogável, lockout progressivo e CSRF em todos os formulários.
- **API administrativa** — catálogo sob `/api/v1`, com OpenAPI gerada do código.
- **Pronto para orquestração** — migrações no boot, sondas separadas, desligamento
  gracioso, logs estruturados e exportação OTLP opcional.

**Não implementado:** MFA, recuperação de senha, exclusão de conta, notificações,
backup automatizado. Ver [known-limitations.md](docs/decisions/known-limitations.md).

## Arquitetura em uma tela

```text
routes/     HTTP puro: formulário/JSON, CSRF, redirect, fragmento vs página
   ↓
services/   Orquestração: consultas concorrentes, paginação, projeção de gráfico
   ↓
repository  TODO o SQL; validação na borda da escrita
   ↓
PostgreSQL  CHECKs como última linha de defesa
```

Um binário único: templates (Askama), migrações (SQLx) e assets estáticos são
**embutidos** nele. Sem frontend separado, sem build de JavaScript, sem microsserviço.

Detalhes: [system-overview.md](docs/architecture/system-overview.md) ·
[component-architecture.md](docs/architecture/component-architecture.md) ·
[data-flow.md](docs/architecture/data-flow.md)

## Decisões que definem o projeto

| Tema | Decisão | ADR |
| --- | --- | --- |
| **Dinheiro** | `Decimal` ↔ `NUMERIC`, escala canônica de 8 casas. Ponto flutuante nunca entra no núcleo financeiro | [0004](docs/adr/0004-decimal-e-numeric-para-dinheiro.md) |
| **Modelo de dados** | `holdings` materializa a posição; `transactions` é o livro-razão imutável | [0005](docs/adr/0005-holdings-materializados-e-livro-razao.md) |
| **Verificação estática** | SQL, templates e traduções checados **em tempo de compilação** | [0003](docs/adr/0003-ssr-com-askama-e-htmx.md) · [0006](docs/adr/0006-sqlx-com-checagem-em-compilacao.md) |
| **Injeção de dependência** | Extratores do Axum: a proteção de uma rota é visível na assinatura do handler | [0002](docs/adr/0002-axum-em-vez-de-rocket.md) |
| **Sessão** | JWT curto + refresh opaco com rotação atômica e revogação real | [0007](docs/adr/0007-sessao-jwt-curto-com-refresh-rotativo.md) |
| **Interface** | SSR + htmx como *progressive enhancement*; CSP fechada sem `unsafe-inline` | [0003](docs/adr/0003-ssr-com-askama-e-htmx.md) |
| **CSS** | Compilado em build-time por executável único — **sem Node, sem npm** | [0010](docs/adr/0010-css-compilado-em-build-time.md) |
| **Mercado** | Snapshot em memória, fora do banco: dado de terceiro não lastreia operação | [0009](docs/adr/0009-snapshot-de-mercado-em-memoria.md) |
| **Observabilidade** | OTLP opt-in, sem custo quando desligada | [0012](docs/adr/0012-observabilidade-opt-in-via-otlp.md) |

## Tecnologias

**axum** · **tokio** · **sqlx** (PostgreSQL, verificado em compilação) · **askama** ·
**rust_decimal** · **password-auth** (argon2) · **jwt-simple** · **subtle** ·
**reqwest** · **tracing** + **OpenTelemetry** · **thiserror** · **serde** ·
**utoipa** (OpenAPI) · **htmx** (vendorado) · **Tailwind CSS** (build-time).

Justificativa de cada uma:
[technology-decisions.md](docs/architecture/technology-decisions.md).

## Requisitos

| Requisito | Versão |
| --- | --- |
| [Rust](https://rustup.rs) | 1.95+ (edition 2024) |
| Docker + Compose | Recente |
| PostgreSQL | 18 (via Docker ou próprio) |

Node e npm **não** são necessários — nem para o CSS.

## Instalação e execução

```bash
docker compose up -d db
```

```bash
cp .env.example .env
```

Edite `.env` e **substitua** `ADMIN_SECRET_KEY` e `JWT_SECRET` — os valores de exemplo
são públicos.

```bash
cargo run
```

Abra <http://localhost:3000>. As migrações são aplicadas no boot.

Stack completo em Docker:

```bash
docker compose --profile app up --build
```

Guia completo: [installation.md](docs/getting-started/installation.md) ·
Problemas conhecidos: [troubleshooting.md](docs/getting-started/troubleshooting.md)

## Configuração

Três variáveis são **obrigatórias** — o serviço não sobe sem elas:

| Variável | Finalidade |
| --- | --- |
| `DATABASE_URL` | Conexão com o Postgres |
| `ADMIN_SECRET_KEY` | Credencial da API administrativa |
| `JWT_SECRET` | Chave de assinatura das sessões |

Outras nove têm padrão sensato. Referência completa, com efeito e risco de cada uma:
[configuration.md](docs/getting-started/configuration.md).

> ⚠️ **Em produção, use exatamente `COOKIE_SECURE=true`.** A comparação é literal:
> `TRUE`, `1` e `yes` resultam em `false`, **silenciosamente** (**DT-04**).

## Testes

**118 testes** em duas camadas — 83 de unidade, 35 de contrato.

```bash
docker compose up -d db && cargo test
```

Só o que não toca banco:

```bash
cargo test --test payload_market --test payload_quotes
```

Os payloads das integrações externas são **reais**, capturados de produção e
versionados: a maior taxa da captura da Coinbase tem 41 dígitos significativos, contra
os 28 da mantissa do `Decimal` — um fixture inventado nunca revelaria isso.

[test-strategy.md](docs/testing/test-strategy.md) ·
[test-catalogue.md](docs/testing/test-catalogue.md) ·
[test-matrix.md](docs/testing/test-matrix.md) — incluindo os riscos **sem** cobertura.

## Estrutura do repositório

```text
src/
  main.rs            # 8 linhas: tokio::main -> App::start()
  lib.rs             # os módulos como BIBLIOTECA — permite tests/ existir
  app.rs             # boot, AppState, router, camadas, sondas, tracing
  config.rs          # lê e valida o ambiente uma vez (fail-fast)
  models.rs · error.rs · i18n.rs
  quotes.rs          # cotações Coinbase -> preços (lastreia dinheiro)
  market.rs          # snapshot CoinGecko em memória (informativo)
  repository.rs      # todo o acesso ao banco + 26 testes
  auth/              # user, session, admin, csrf, throttle
  services/          # portfolio: orquestração da carteira
  routes/            # api (JSON), frontend (SSR), flash
tests/               # suíte de contrato; payloads reais versionados
templates/ · migrations/ · static/ · styles/ · docs/
```

## Segurança

Controles: CSP sem `unsafe-inline`, CSRF, lockout progressivo, argon2, refresh token
rotativo com revogação real, comparação de segredo em tempo constante, queries
parametrizadas, erros 5xx censurados, container sem privilégio, zero dependências npm.

> **Isso não torna o sistema seguro** — torna-o um sistema com controles conhecidos e
> limites documentados. Riscos residuais, ameaças e ações prioritárias em
> [threat-model.md](docs/security/threat-model.md).

Para relatar uma vulnerabilidade: [SECURITY.md](SECURITY.md).

## Observabilidade

Logs estruturados (JSON opcional) com `request_id` por requisição, traces e o
histograma `http.server.request.duration` exportáveis via OTLP — **opt-in**: sem
`OTEL_EXPORTER_OTLP_ENDPOINT`, nenhuma tentativa de conexão.

Sondas separadas: `/healthz` (liveness, não toca o banco) e `/readyz` (readiness).

[observability.md](docs/operations/observability.md)

## Limitações conhecidas

Registradas para que a ausência não seja confundida com omissão:

- **Instância única** — lockout e snapshot de mercado vivem em memória do processo.
- **Sem backup implementado** — `docker compose down -v` é perda total (**DT-05**).
- **Nenhum teste executa JavaScript** — o htmx é verificado pelo HTML emitido.
- **Reversão de migração nunca testada** — os 11 `.down.sql` existem, nenhum é
  executado.
- **Cobertura de testes não medida** — não há ferramenta configurada.
- **Sem criptografia em repouso.**

Completas: [known-limitations.md](docs/decisions/known-limitations.md) ·
Débitos com prioridade: [technical-debt.md](docs/decisions/technical-debt.md)

## Roadmap

As cinco fases planejadas estão **concluídas** —
[roadmap.md](docs/decisions/roadmap.md) é hoje um histórico. O trabalho pendente está
registrado como débito técnico, não como roadmap.

## Contribuindo

[CONTRIBUTING.md](CONTRIBUTING.md). Especialmente bem-vindas: as correções de
prioridade alta em [technical-debt.md](docs/decisions/technical-debt.md) — três delas
são de baixo esforço.

## Licença

> ⚠️ **Este projeto ainda não tem licença definida.**
>
> Um repositório público sem licença significa **todos os direitos reservados**: o
> código não pode ser legalmente usado, modificado ou redistribuído por terceiros.
>
> A titularidade **foi verificada**: os [Termos de Uso da DIO](https://www.dio.me/terms)
> declaram que a plataforma *"não clama propriedade"* do conteúdo do usuário, e o que
> ela reivindica como "Conteúdo" é o próprio material didático — não a implementação do
> aluno. Os editais Santander/DIO não tratam de propriedade intelectual.
>
> Falta apenas verificar as licenças das dependências (`cargo license`) e escolher entre
> **MIT**, **Apache-2.0** ou o duplo `MIT OR Apache-2.0`, convenção do ecossistema Rust.
>
> Análise completa: [licensing.md](docs/decisions/licensing.md).

## Notas de ambiente (Windows)

- **TLS do cargo:** `.cargo/config.toml` já desativa apenas a checagem de revogação,
  que falha com `CRYPT_E_NO_REVOCATION_CHECK` em algumas redes.
- **`jwt-simple` sem `cmake`:** configurado com `pure-rust`.
- **Postgres 18:** o volume é montado em `/var/lib/postgresql` (convenção da imagem
  18+).
- **Proxy corporativo com inspeção TLS:** ver
  [troubleshooting.md](docs/getting-started/troubleshooting.md) §3.
