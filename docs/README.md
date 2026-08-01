# Documentação do wallet-live

Índice da documentação técnica. Cada documento declara seu **objetivo** e **escopo**
no topo, e cita os arquivos e símbolos do código que sustentam suas afirmações.

## Por onde começar

| Se você quer… | Comece por |
| --- | --- |
| **Rodar o projeto** | [getting-started/installation.md](getting-started/installation.md) |
| **Entender como funciona** | [architecture/system-overview.md](architecture/system-overview.md) |
| **Entender por que é assim** | [adr/](adr/) — 12 decisões arquiteturais |
| **Consumir a API** | [api/endpoints.md](api/endpoints.md) |
| **Operar o sistema** | [operations/runbooks.md](operations/runbooks.md) |
| **Contribuir** | [../CONTRIBUTING.md](../CONTRIBUTING.md) |
| **Saber o que não funciona** | [decisions/known-limitations.md](decisions/known-limitations.md) |

## Primeiros passos

| Documento | Conteúdo |
| --- | --- |
| [installation.md](getting-started/installation.md) | Requisitos, instalação, primeira execução, estrutura do repositório |
| [configuration.md](getting-started/configuration.md) | As 13 variáveis de ambiente, efeito, risco e onde são lidas |
| [troubleshooting.md](getting-started/troubleshooting.md) | Problemas conhecidos de build, banco, Docker e interface |

## Arquitetura

| Documento | Conteúdo |
| --- | --- |
| [system-overview.md](architecture/system-overview.md) | Como o sistema é montado, etapa por etapa, a partir do código |
| [component-architecture.md](architecture/component-architecture.md) | Ficha dos 18 componentes; regras de dependência entre camadas |
| [data-flow.md](architecture/data-flow.md) | Diagramas de contexto, componentes, sequência, autenticação, erro e deploy |
| [technology-decisions.md](architecture/technology-decisions.md) | Justificativa das 12 tecnologias, com alternativas e limites |

## Decisões arquiteturais (ADR)

[Índice completo](adr/README.md) — 12 decisões no formato canônico.

| ADR | Decisão |
| --- | --- |
| [0001](adr/0001-rust-como-linguagem-unica.md) | Rust como linguagem única, com alvo de biblioteca |
| [0002](adr/0002-axum-em-vez-de-rocket.md) | axum em vez de Rocket |
| [0003](adr/0003-ssr-com-askama-e-htmx.md) | SSR com Askama + htmx, sem SPA |
| [0004](adr/0004-decimal-e-numeric-para-dinheiro.md) | `Decimal` ↔ `NUMERIC`, escala de 8 casas |
| [0005](adr/0005-holdings-materializados-e-livro-razao.md) | `holdings` materializado + livro-razão imutável |
| [0006](adr/0006-sqlx-com-checagem-em-compilacao.md) | sqlx com verificação em compilação |
| [0007](adr/0007-sessao-jwt-curto-com-refresh-rotativo.md) | JWT curto + refresh rotativo |
| [0008](adr/0008-autorizacao-por-papel-e-credencial-de-servico.md) | Autorização por papel ou credencial de serviço |
| [0009](adr/0009-snapshot-de-mercado-em-memoria.md) | Snapshot de mercado em memória |
| [0010](adr/0010-css-compilado-em-build-time.md) | CSS compilado em build-time, sem Node |
| [0011](adr/0011-versionamento-da-api-por-caminho.md) | Versionamento de API por caminho |
| [0012](adr/0012-observabilidade-opt-in-via-otlp.md) | Observabilidade opt-in via OTLP |

## API e contratos

| Documento | Conteúdo |
| --- | --- |
| [api-overview.md](api/api-overview.md) | Inventário de superfícies, versionamento, cookies, jobs, convenções |
| [endpoints.md](api/endpoints.md) | As 21 rotas HTTP, com parâmetros e respostas |
| [authentication.md](api/authentication.md) | Sessão, CSRF, credencial de serviço, lockout |
| [payloads.md](api/payloads.md) | Campo a campo, entrada e saída, incluindo integrações externas |
| [errors.md](api/errors.md) | As 21 variantes de `AppError` e o mapeamento HTTP |

## Dados

| Documento | Conteúdo |
| --- | --- |
| [data-model.md](data/data-model.md) | Modelo de domínio, invariantes, ciclo de vida, crescimento |
| [database-schema.md](data/database-schema.md) | As 6 tabelas, restrições, índices, tipos |
| [data-dictionary.md](data/data-dictionary.md) | Os 28 campos: origem, sensibilidade, retenção |
| [migrations.md](data/migrations.md) | As 11 migrações, reversibilidade, procedimento |

## Testes

| Documento | Conteúdo |
| --- | --- |
| [test-strategy.md](testing/test-strategy.md) | Níveis, dublês, execução, limites — **e a contagem canônica** |
| [test-catalogue.md](testing/test-catalogue.md) | Os 118 testes, um a um, com o risco que reduzem |
| [test-matrix.md](testing/test-matrix.md) | Leitura por risco, incluindo os **sem** cobertura |
| [test-plan.md](testing/test-plan.md) | Objetivos, ambientes, critérios, frequência |
| [coverage.md](testing/coverage.md) | Estado da medição (ausente) e proposta |
| [test-report-template.md](testing/test-report-template.md) | Modelo de relatório de liberação |

## Segurança

| Documento | Conteúdo |
| --- | --- |
| [security-architecture.md](security/security-architecture.md) | Fronteiras, superfícies, controles por camada, riscos residuais |
| [threat-model.md](security/threat-model.md) | Ativos, ameaças, cenários e ações prioritárias |
| [secrets-management.md](security/secrets-management.md) | Os 4 segredos, ciclo de vida, rotação, vazamento |

Ver também [../SECURITY.md](../SECURITY.md) para divulgação de vulnerabilidades.

## Operação

| Documento | Conteúdo |
| --- | --- |
| [deployment.md](operations/deployment.md) | Build, imagem, boot, ciclo de vida, rollback |
| [runbooks.md](operations/runbooks.md) | 12 incidentes, com diagnóstico e recuperação |
| [observability.md](operations/observability.md) | Logs, traces, métricas, sondas e correlação |
| [backup-and-recovery.md](operations/backup-and-recovery.md) | Estado (ausente), procedimentos e recomendações |

## Desenvolvimento

| Documento | Conteúdo |
| --- | --- |
| [development-environment.md](development/development-environment.md) | Ciclo diário e artefatos a regenerar |
| [coding-standards.md](development/coding-standards.md) | Convenções e os invariantes que **nenhuma ferramenta verifica** |
| [commit-conventions.md](development/commit-conventions.md) | Conventional Commits, SemVer, release |
| [dependency-management.md](development/dependency-management.md) | As 30 dependências diretas, auditoria, atualização |

## Decisões e estado do projeto

| Documento | Conteúdo |
| --- | --- |
| [technical-debt.md](decisions/technical-debt.md) | 24 débitos com prioridade — **índice dos `DT-XX`** |
| [known-limitations.md](decisions/known-limitations.md) | O que o sistema **não** faz, e por quê |
| [licensing.md](decisions/licensing.md) | Análise de licença — **decisão pendente** |
| [roadmap.md](decisions/roadmap.md) | Histórico das 5 fases entregues |

## Contexto acadêmico

| Documento | Conteúdo |
| --- | --- |
| [course-delivery.md](delivery/course-delivery.md) | Matriz de requisitos da entrega e roteiro de demonstração |
| [aprendizado/README.md](aprendizado/README.md) | Aprendizados técnicos aplicados ao projeto, sem reproduzir conteúdo didático |

---

## Convenções desta documentação

| Convenção | Detalhe |
| --- | --- |
| **Idioma** | Português formal |
| **Rastreabilidade** | Afirmação técnica cita arquivo **e símbolo** — nunca número de linha, que muda a cada commit |
| **Motivo confirmado vs. inferido** | Onde o repositório registra o motivo, ele é citado; onde não, a análise é marcada como inferida |
| **Ausências declaradas** | O que não existe é registrado como tal, não omitido |
| **Sem valores reais** | Exemplos usam valores fictícios; nenhuma credencial, token ou chave real aparece |

### Onde encontrar cada tipo de informação

| Pergunta | Documento |
| --- | --- |
| Como funciona? | `architecture/` |
| Por que assim? | `adr/` |
| O que responde nesta rota? | `api/` |
| O que significa este campo? | `data/` |
| Isto é testado? | `testing/` |
| É seguro? | `security/` |
| Como diagnosticar? | `operations/` |
| Como contribuir? | `development/` + [../CONTRIBUTING.md](../CONTRIBUTING.md) |
| O que falta? | `decisions/` |
