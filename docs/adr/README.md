# Architecture Decision Records

## Objetivo

Registrar as decisões arquiteturais relevantes do `wallet-live` em formato
citável e revisável: contexto, restrições, alternativas, decisão, fundamentação,
consequências e critérios de revisão.

## Escopo

Coberto: 12 decisões com impacto estrutural — as que, se revertidas, exigiriam
mudança em mais de um módulo. Não coberto: decisões triviais (escolha de nome de
variável, formatação, biblioteca utilitária sem alternativa relevante) e a
narrativa de como o sistema funciona hoje (ver
[../architecture/system-overview.md](../architecture/system-overview.md)).

## Aviso sobre a origem destes ADRs

> Estes ADRs foram escritos **retroativamente**, a partir do código, dos
> comentários, das mensagens de migração e do histórico de commits. O projeto não
> mantinha ADRs durante a construção.
>
> Onde o motivo original está registrado no repositório, ele é citado e marcado
> como **motivo confirmado**. Onde não está, a fundamentação é explicitamente
> apresentada como **análise técnica da implementação atual** — nenhum ADR aqui
> inventa uma deliberação histórica que não aconteceu.
>
> Consequência prática: o campo **Opções consideradas** distingue as alternativas
> que foram **de fato avaliadas** (com evidência) das que são comparação técnica
> *post hoc*.

## Índice

| ADR | Decisão | Status | Confirmado? |
| --- | --- | --- | --- |
| [0001](0001-rust-como-linguagem-unica.md) | Rust como linguagem única, em um crate com alvo de biblioteca | Aceita | Parcial — o contexto (curso) é confirmado; a avaliação de alternativas não aconteceu |
| [0002](0002-axum-em-vez-de-rocket.md) | axum em vez de Rocket | Aceita | **Sim** — comparação documentada |
| [0003](0003-ssr-com-askama-e-htmx.md) | SSR com Askama + htmx, sem SPA | Aceita | **Sim** |
| [0004](0004-decimal-e-numeric-para-dinheiro.md) | `Decimal` ↔ `NUMERIC` com escala canônica de 8 casas | Aceita | **Sim** — migração + incidente de produção |
| [0005](0005-holdings-materializados-e-livro-razao.md) | `holdings` materializado + `transactions` imutável | Aceita | **Sim** — mensagem da migração |
| [0006](0006-sqlx-com-checagem-em-compilacao.md) | sqlx com verificação em compilação e cache offline versionado | Aceita | **Sim** |
| [0007](0007-sessao-jwt-curto-com-refresh-rotativo.md) | JWT de acesso curto + refresh token opaco rotativo | Aceita | **Sim** |
| [0008](0008-autorizacao-por-papel-e-credencial-de-servico.md) | Autorização por papel de sessão **ou** credencial de serviço | Aceita | **Sim** |
| [0009](0009-snapshot-de-mercado-em-memoria.md) | Snapshot de mercado em memória, fora do banco | Aceita | **Sim** |
| [0010](0010-css-compilado-em-build-time.md) | CSS compilado em build-time, sem Node | Aceita | **Sim** — substitui decisão anterior (Play CDN) |
| [0011](0011-versionamento-da-api-por-caminho.md) | Versionamento de API por caminho, com alias de compatibilidade | Aceita | **Sim** |
| [0012](0012-observabilidade-opt-in-via-otlp.md) | Observabilidade opt-in via OTLP, sem custo quando desligada | Aceita | **Sim** |

## Decisões que ainda não têm ADR

Registradas aqui para que a ausência seja visível em vez de silenciosa:

| Decisão | Onde está documentada hoje | Por que não virou ADR |
| --- | --- | --- |
| Escolha do PostgreSQL | [../architecture/technology-decisions.md](../architecture/technology-decisions.md) §10 | Motivo histórico não registrado; a análise disponível não sustenta um ADR honesto sem inventar deliberação |
| Manter `BIGSERIAL` em vez de UUIDv7 | [../decisions/roadmap.md](../decisions/roadmap.md), Fase 3 | **Esta é a decisão mais próxima de merecer um ADR** — tem contexto, alternativa avaliada e critério de revisão explícito. Recomenda-se promovê-la a ADR-0013 |
| Estratégia de testes em duas camadas | [../testing/test-strategy.md](../testing/test-strategy.md) | Documentada em profundidade no lugar próprio; um ADR duplicaria |
| Licenciamento | [../decisions/licensing.md](../decisions/licensing.md) | **Decisão ainda não tomada** — depende de confirmação de titularidade |

## Formato

Todo ADR novo deve seguir a estrutura abaixo, sem omitir seções (uma seção sem
conteúdo relevante recebe "Não aplicável" com uma linha de justificativa):

```markdown
# ADR-XXXX: Título da decisão

## Status
Proposta | Aceita | Substituída por ADR-YYYY | Rejeitada | Descontinuada

## Contexto
## Restrições
## Opções consideradas
## Decisão
## Fundamentação
## Consequências positivas
## Consequências negativas
## Riscos
## Evidências
## Critérios de revisão
```

Numeração sequencial, sem reuso. Um ADR **nunca é editado** para mudar a decisão:
cria-se um novo com status `Aceita` e marca-se o anterior como `Substituída por
ADR-YYYY`.
