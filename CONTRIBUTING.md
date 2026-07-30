# Contribuindo com o wallet-live

## Antes de começar

> **Aviso sobre licenciamento.** Este repositório **ainda não tem licença definida**,
> o que torna juridicamente ambíguo como uma contribuição seria licenciada. A
> titularidade do código já foi verificada (é do autor); falta escolher e publicar a
> licença. Ver [docs/decisions/licensing.md](docs/decisions/licensing.md). Se pretende
> contribuir com código, vale abrir uma issue antes para resolver esse ponto.

Este é um projeto de **autor único**, originado do bootcamp Santander 2026 — Rust AI
Developer (DIO). Contribuições são bem-vindas, e este documento descreve o processo
real, não um processo corporativo que não existe.

## Preparar o ambiente

```bash
git clone https://github.com/ESousa97/wallet-live.git && cd wallet-live
```

```bash
docker compose up -d db
```

```bash
cp .env.example .env
```

Edite `.env` e **substitua** `ADMIN_SECRET_KEY` e `JWT_SECRET` — os valores de exemplo
(`change-me`) são públicos.

```bash
cargo run
```

Detalhes em
[docs/getting-started/installation.md](docs/getting-started/installation.md).

## Fluxo de contribuição

1. **Abra uma issue** antes de mudanças significativas, para alinhar a abordagem.
2. **Crie um branch:**

   ```bash
   git switch -c feat/nome-da-mudanca
   ```

3. **Implemente**, seguindo
   [docs/development/coding-standards.md](docs/development/coding-standards.md).
4. **Escreva o teste** — ver a política abaixo.
5. **Documente** o teste em
   [docs/testing/test-catalogue.md](docs/testing/test-catalogue.md).
6. **Verifique** com a lista da próxima seção.
7. **Abra o pull request**, explicando *o quê* e *por quê*.

O CI roda automaticamente nos quatro jobs: `lint`, `test`, `audit` e `docker`.

## Antes de abrir o PR

```bash
cargo fmt --all
```

```bash
cargo clippy --all-targets -- -D warnings
```

```bash
cargo test
```

### Lista de verificação

| # | Item | Quando se aplica |
| --- | --- | --- |
| 1 | `cargo fmt --all` | Sempre |
| 2 | `cargo clippy -D warnings` sem apontamentos | Sempre |
| 3 | `cargo test` — os 118 passando | Sempre |
| 4 | `cargo sqlx prepare` | Query SQL nova ou alterada |
| 5 | Recompilar `static/app.css` (Tailwind **4.3.3**) | Classe CSS nova |
| 6 | `cargo insta review` | Formato de resposta da API alterado |
| 7 | **`round_dp(MONEY_SCALE)`** na escrita | Valor monetário novo |
| 8 | **`ROUND(..., 8)`** no agregado | Query nova que soma ou multiplica dinheiro |
| 9 | Sem `<style>`/`<script>` inline | Template novo |
| 10 | `#[instrument(skip_all)]` | Handler que recebe formulário |
| 11 | Proteção visível na assinatura | Rota nova |
| 12 | Teste documentado no catálogo | Teste novo |
| 13 | Migração `up` **e** `down` | Mudança de schema |

> **Os itens 7 e 8 são os que nenhuma ferramenta verifica**, e o 8 já causou um
> incidente de produção: uma query de agregado sem `ROUND` derruba a tela da carteira
> com `value not representable`. Ver
> [ADR-0004](docs/adr/0004-decimal-e-numeric-para-dinheiro.md).

## Política de testes

**Toda mudança de comportamento precisa de teste.**

| Tipo de mudança | Nível exigido |
| --- | --- |
| **Caminho de dinheiro** | `#[sqlx::test]` contra **Postgres real** — nunca dublê |
| Orquestração de tela | Unidade com `FakeRepository` |
| Autenticação, autorização, CSRF | Contrato, pelo router real (`tests/http_*.rs`) |
| Renderização | Unidade em `routes/frontend.rs` |
| Formato de resposta da API | Snapshot `insta` |
| Parse de payload externo | Contrato, contra o payload real versionado |
| Correção de defeito | **Teste que falharia antes da correção** |

### Como nomear

Nomes de teste são **frases que enunciam o invariante**, não o método testado:

```text
buy_rejects_when_balance_is_insufficient
partial_sell_keeps_remaining_units
forms_without_a_matching_csrf_token_are_refused
```

Uma falha nesse nome já diz o que quebrou, sem abrir o arquivo. **Prefira inglês** —
é a maioria no repositório.

### Documente o motivo

Todo teste novo entra em
[docs/testing/test-catalogue.md](docs/testing/test-catalogue.md) com **o que trava** e
**por que existe**.

> Um teste cujo motivo não está escrito é um teste que alguém apaga no primeiro
> refactor em que ele incomoda.

Evite justificativa genérica ("garante que funciona"). Prefira específica:

> Verifica que uma operação cujo total arredonda a zero não move caixa nem posição.
> Sem isso, o sistema viraria um moedor de unidades grátis.

## Convenções de commit

O repositório segue **Conventional Commits**, em inglês:

```text
feat: add withdrawal support to the wallet

O corpo explica o QUÊ e o PORQUÊ, não o COMO.
```

Tipos: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`, `chore`.
Use `!` antes dos dois-pontos para mudança incompatível.

Detalhes em
[docs/development/commit-conventions.md](docs/development/commit-conventions.md).

## Documentação

Mudança que altere comportamento observável precisa atualizar a documentação
correspondente:

| Mudou | Atualize |
| --- | --- |
| Rota | [docs/api/endpoints.md](docs/api/endpoints.md) |
| Payload | [docs/api/payloads.md](docs/api/payloads.md) |
| Variante de erro | [docs/api/errors.md](docs/api/errors.md) |
| Variável de ambiente | [configuration.md](docs/getting-started/configuration.md) e `.env.example` |
| Schema | [database-schema.md](docs/data/database-schema.md), [data-dictionary.md](docs/data/data-dictionary.md), [migrations.md](docs/data/migrations.md) |
| Teste | [test-catalogue.md](docs/testing/test-catalogue.md) |
| **Decisão arquitetural** | **Novo ADR** em [docs/adr/](docs/adr/) |
| Dependência | [dependency-management.md](docs/development/dependency-management.md) |

### Regras de escrita

| Regra | Detalhe |
| --- | --- |
| Português formal | Objetivo, verificável |
| **Rastreabilidade** | Cite **arquivo e símbolo**, nunca número de linha |
| **Sem linguagem promocional** | Nada de "robusto", "poderoso", "de ponta" |
| **Declare ausências** | O que não existe é registrado como tal |
| **Sem valores reais** | Nenhuma credencial, token ou chave — use valores fictícios marcados |
| Motivo confirmado vs. inferido | Não invente deliberação histórica que não aconteceu |

## Quando abrir um ADR

Se a mudança envolve uma decisão que, revertida, exigiria alterar mais de um módulo.
Use o formato de [docs/adr/README.md](docs/adr/README.md), com o próximo número livre.

Não abra ADR para detalhe trivial.

## O que é especialmente bem-vindo

Os débitos técnicos de **prioridade alta**, que são de baixo esforço e alto retorno:

| ID | Contribuição | Esforço |
| --- | --- | --- |
| **DT-04** | Corrigir o parsing de `COOKIE_SECURE` (aceitar `true`/`1`/`yes`, sem distinção de caixa) | **Baixo** |
| **DT-07** | Exigir comprimento mínimo e recusar valores de exemplo nos segredos | **Baixo** |
| **DT-23** | Sanitizar a `DATABASE_URL` antes de registrá-la em log | **Baixo** |
| **DT-09** | Teste que aplique e reverta as 11 migrações | Médio |
| **DT-10** | Tornar `Config::from_env` testável e testá-la | Médio |
| **DT-06** | Traduzir nome de ativo duplicado para `400` | Baixo |
| **DT-22** | Contador de falhas de sincronização de cotações | Baixo |

Lista completa em
[docs/decisions/technical-debt.md](docs/decisions/technical-debt.md).

## Segurança

**Não abra issue pública** para vulnerabilidade. Ver [SECURITY.md](SECURITY.md).

## Código de conduta

Não há documento formal — o projeto tem autor único e nenhuma comunidade estabelecida.
A expectativa é a usual: discussão técnica, objetiva e respeitosa. Se o projeto ganhar
colaboradores recorrentes, um `CODE_OF_CONDUCT.md` deve ser adotado.

## Dúvidas

Abra uma issue. Para entender o projeto antes de contribuir:

| Objetivo | Documento |
| --- | --- |
| Como funciona | [system-overview.md](docs/architecture/system-overview.md) |
| Por que é assim | [docs/adr/](docs/adr/) |
| O que já é conhecido como falho | [technical-debt.md](docs/decisions/technical-debt.md) |
| O que não faz, por decisão | [known-limitations.md](docs/decisions/known-limitations.md) |
