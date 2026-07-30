# Convenções de commit e versionamento

## Objetivo

Documentar a convenção de mensagens de commit **efetivamente praticada** no
repositório, a política de versionamento e o processo de release.

## Escopo

Coberto: formato de commit, tipos em uso, versionamento, changelog e release. Não
coberto: padrões de código (ver [coding-standards.md](coding-standards.md)) e
processo de contribuição (ver [../../CONTRIBUTING.md](../../CONTRIBUTING.md)).

---

## 1. Conventional Commits — adotado na prática, agora registrado

O repositório **já segue** Conventional Commits desde o primeiro commit, mas a
convenção nunca esteve documentada. Este documento registra a prática existente em
vez de propor uma nova.

Distribuição real dos 36 commits:

| Tipo | Commits | Uso |
| --- | ---: | --- |
| `feat:` | 23 | Funcionalidade nova |
| `docs:` | 5 | Só documentação |
| `refactor:` | 3 | Reestruturação sem mudança de comportamento |
| `ci:` | 2 | Pipeline |
| `perf!:` | 1 | Desempenho, **com quebra** |
| `fix:` | 1 | Correção de defeito |

### Formato

```text
<tipo>[!]: <descrição no imperativo, minúscula, sem ponto final>

[corpo explicando O QUÊ e POR QUÊ, não COMO]
```

### Tipos

| Tipo | Quando usar | Impacto em versão |
| --- | --- | --- |
| `feat` | Funcionalidade nova | MINOR |
| `fix` | Correção de defeito | PATCH |
| `perf` | Melhoria de desempenho | PATCH |
| `refactor` | Reestruturação sem mudar comportamento | PATCH |
| `docs` | Só documentação | Nenhum |
| `test` | Só testes | Nenhum |
| `ci` | Pipeline | Nenhum |
| `chore` | Manutenção (dependências, configuração) | Nenhum |

O `!` antes dos dois-pontos marca mudança incompatível: `perf!: ...`.

### O estilo praticado

As mensagens deste repositório têm uma característica consistente que vale preservar:
**o assunto descreve o resultado, não a implementação**. Exemplos reais:

```text
feat: market panel, a library target and the integration suite it enables
perf!: compile Tailwind at build time and close the CSP
fix: cap money scale at 8 decimal places end to end
feat: page-reload-free wallet operations via htmx partials
```

Repare que `perf!: compile Tailwind at build time and close the CSP` nomeia
**as duas consequências** — o ganho de desempenho e o fechamento da CSP —, e o `!`
sinaliza que algo incompatível mudou.

**O idioma dos commits é inglês**, enquanto a documentação é em português. É a prática
estabelecida; mantenha-a por consistência com o histórico.

### O corpo explica o porquê

O corpo é onde a decisão fica registrada. É de lá que boa parte desta documentação foi
reconstruída. Um corpo útil responde:

- Que problema isto resolve?
- Que alternativa foi descartada, e por quê?
- Que efeito colateral o leitor precisa conhecer?

## 2. Versionamento

### Estado atual

| Item | Estado |
| --- | --- |
| Versão em `Cargo.toml` | `0.1.0` |
| Tags no repositório | **Nenhuma** |
| Releases publicados | Nenhum |
| Versão da API (OpenAPI) | `1.0.0` |

> **A versão `0.1.0` nunca foi incrementada**, apesar de 36 commits com mudanças
> substanciais — inclusive uma quebra explícita (`perf!`). Registrado como débito
> técnico.

### Semantic Versioning

O projeto **deve** adotar SemVer (`MAJOR.MINOR.PATCH`):

| Incremento | Quando |
| --- | --- |
| MAJOR | Mudança incompatível — commit com `!` |
| MINOR | Funcionalidade nova retrocompatível — `feat` |
| PATCH | Correção retrocompatível — `fix`, `perf`, `refactor` |

**Enquanto a versão for `0.x`**, o SemVer permite que qualquer MINOR quebre
compatibilidade — o que é adequado ao estágio atual do projeto.

### O que "compatibilidade" significa aqui

Três superfícies distintas, com estabilidade diferente:

| Superfície | Estabilidade | Quebra significa |
| --- | --- | --- |
| **API JSON** (`/api/v1`) | **Contratual** | Campo renomeado, tipo alterado, rota removida. Exige `/api/v2` |
| **Schema do banco** | Interna, mas migrável | Migração destrutiva sem caminho de reversão |
| **Variáveis de ambiente** | Semi-contratual | Variável obrigatória nova, ou mudança de padrão |
| Interface HTML | Não contratual | Não há consumidor programático |

O contrato JSON é congelado por snapshots `insta`, então uma quebra **não passa
despercebida**: exige `cargo insta review` explícito.

## 3. Changelog

### Keep a Changelog

O `CHANGELOG.md` segue o formato [Keep a Changelog](https://keepachangelog.com), com
as seções:

`Adicionado` · `Alterado` · `Corrigido` · `Removido` · `Descontinuado` · `Segurança`

### Por que não é gerado do histórico

O changelog **não é uma cópia do `git log`**. Um histórico de commits registra o que
foi feito; um changelog registra **o que mudou para quem usa**.

| `git log` | `CHANGELOG.md` |
| --- | --- |
| 36 entradas | Agrupado por versão |
| Ordem cronológica | Agrupado por natureza da mudança |
| Inclui `docs:`, `ci:`, `refactor:` | Só o que afeta quem usa ou opera |
| Escrito para o autor | Escrito para o leitor |

`docs:` e `ci:` normalmente **não** entram no changelog — não mudam nada para quem usa
o software.

## 4. Processo de release

Não há release publicado. O processo abaixo é o **recomendado** para o primeiro:

1. **Verificar os critérios de saída** de
   [../testing/test-plan.md](../testing/test-plan.md) §6.
2. **Atualizar `CHANGELOG.md`**, movendo o conteúdo de `[Não publicado]` para a nova
   versão, com data.
3. **Atualizar a versão** em `Cargo.toml` e confirmar que o `Cargo.lock` acompanhou.
4. **Produzir o relatório de testes** conforme
   [../testing/test-report-template.md](../testing/test-report-template.md).
5. **Commit e tag:**

   ```bash
   git commit -am "chore: release v0.2.0"
   ```

   ```bash
   git tag -a v0.2.0 -m "v0.2.0"
   ```

6. **Publicar:**

   ```bash
   git push && git push --tags
   ```

### Antes de qualquer release

| Verificação | Comando |
| --- | --- |
| Suíte completa | `cargo test` |
| Formatação | `cargo fmt --all --check` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Build de release | `cargo build --release` |
| Imagem | `docker build .` |
| Cache SQLx | `cargo sqlx prepare --check` |
| CSS em dia | Recompilar e `diff` |
| Snapshots pendentes | `find . -name '*.snap.new' -not -path './target/*'` |
| Auditoria | `cargo audit` |
| **Backup antes de migração** | Ver [../operations/backup-and-recovery.md](../operations/backup-and-recovery.md) §6 |

## 5. Estratégia de ramificação

Prática atual: **trunk-based** em `master`, com autor único.

| Aspecto | Prática |
| --- | --- |
| Branch principal | `master` |
| Branches de feature | Não usadas atualmente |
| Merge | Commits diretos |
| CI | Roda em push para `master` e em todo PR |

Para trabalho colaborativo, o modelo recomendado é branch por mudança + PR:

```bash
git switch -c feat/nome-da-mudanca
```

O CI já está configurado para rodar em pull requests.

## 6. Migração incompatível: o que documentar

Toda mudança incompatível (`!`) deve registrar, no corpo do commit **e** no changelog:

1. **O que quebrou** — qual contrato deixou de valer.
2. **Quem é afetado** — consumidores da API, quem opera, ou ambos.
3. **Como migrar** — o passo concreto.
4. **Se há caminho de reversão.**

O commit `perf!: compile Tailwind at build time and close the CSP` é o exemplo real: a
quebra foi a saída do Play CDN, com efeito em quem dependia do comportamento anterior
de estilo em runtime.

## 7. Débitos registrados

| # | Débito | Impacto |
| --- | --- | --- |
| 1 | Versão nunca incrementada (`0.1.0` em 36 commits) | Não há como referenciar uma versão específica |
| 2 | Sem tags | Não há ponto de retorno identificável |
| 3 | `CHANGELOG.md` criado retroativamente | As entradas históricas são reconstruções, não registros contemporâneos |
| 4 | Sem política de descontinuação da API | Ver [ADR-0011](../adr/0011-versionamento-da-api-por-caminho.md) |

Consolidados em [../decisions/technical-debt.md](../decisions/technical-debt.md).

## 8. Evidências

```text
- git log                    (36 commits, todos em Conventional Commits)
- Cargo.toml                 (version = "0.1.0")
- src/routes/api.rs          · ApiDoc (info.version = "1.0.0")
- src/routes/snapshots/      (contrato congelado por insta)
- .github/workflows/ci.yml   (gatilhos: push em master e pull_request)
- CHANGELOG.md
```
