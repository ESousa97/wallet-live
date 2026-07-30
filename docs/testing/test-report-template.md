# Modelo de relatório de testes

## Objetivo

Definir o formato padrão de um relatório de execução de testes, para que uma
liberação de versão tenha registro verificável em vez de uma impressão.

## Escopo

Coberto: o modelo, as regras de preenchimento e um exemplo. Não coberto: quais testes
existem (ver [test-catalogue.md](test-catalogue.md)) e critérios de liberação (ver
[test-plan.md](test-plan.md) §6).

---

## 1. Quando produzir um relatório

| Situação | Relatório? |
| --- | :---: |
| Commit de desenvolvimento | Não |
| Pull request | Não — o CI é o registro |
| **Release / tag de versão** | **Sim** |
| **Mudança em caminho de dinheiro** | **Sim** |
| **Após incidente em produção** | **Sim** |
| Mudança só de documentação | Não |

Relatórios ficam em `docs/testing/reports/` com o nome
`relatorio-vX.Y.Z-AAAA-MM-DD.md`. **O diretório ainda não existe** — será criado na
primeira liberação formal.

## 2. Regras de preenchimento

1. **Toda afirmação precisa de número ou comando.** "Todos os testes passaram" sem a
   contagem não é registro.
2. **Falha não corrigida é registrada, não omitida.** Um relatório que só documenta
   sucesso não serve para nada.
3. **A conclusão não usa termos vagos.** Nunca "aparentemente está tudo correto" ou
   "aparenta estabilidade". Use critérios objetivos, nomeando o que foi validado e o
   que **não** foi.
4. **Riscos residuais são obrigatórios.** A seção nunca fica vazia — este projeto tem
   10 riscos sem cobertura permanentes, listados em
   [test-matrix.md](test-matrix.md) §11.
5. **Nenhum segredo, credencial ou dado pessoal real** aparece em evidência. Trechos
   de log devem ser sanitizados.

## 3. Modelo

````markdown
# Relatório de Testes — Versão X.Y.Z

## Identificação

- **Versão:** X.Y.Z
- **Commit:** `<sha completo>`
- **Branch:** master
- **Data:** AAAA-MM-DD
- **Ambiente:** local | CI (GitHub Actions, ubuntu-latest)
- **Responsável:** <nome>
- **Sistema operacional:** <ex.: Windows 11 Pro 26200 / Ubuntu 24.04>
- **Dependências relevantes:**
  - Rust: `<cargo --version>`
  - PostgreSQL: 18 (Docker)
  - Docker: `<docker --version>`

## Escopo

<O que esta execução cobriu e o que ficou deliberadamente de fora.
Exemplo: "Suíte completa (118 testes) mais lint e auditoria. Não inclui
teste de carga nem verificação em navegador — ausentes por desenho."">

## Testes executados

| Camada | Comando | Testes | Resultado |
| --- | --- | ---: | --- |
| Unidade | `cargo test --lib` | 83 | |
| Contrato | `cargo test --tests` | 35 | |
| **Total** | `cargo test` | **118** | |

Verificações complementares:

| Verificação | Comando | Resultado |
| --- | --- | --- |
| Formatação | `cargo fmt --all --check` | |
| Lint | `cargo clippy --all-targets -- -D warnings` | |
| Cache SQLx | `cargo sqlx prepare --check` | |
| Frescor do CSS | recompilar e `diff` com `static/app.css` | |
| Auditoria | `cargo audit` | |
| Build de release | `cargo build --release` | |
| Imagem | `docker build .` | |

## Resultados

<Saída resumida. Para sucesso total, a linha final do cargo basta:
`test result: ok. N passed; 0 failed; 0 ignored`.>

## Falhas encontradas

| # | Teste | Sintoma | Causa | Status |
| --- | --- | --- | --- | --- |

<Se não houve, escrever "Nenhuma." — não apagar a seção.>

## Evidências

<Comandos executados e saída relevante, sanitizados. Referenciar a
execução do CI pela URL quando aplicável.>

## Riscos residuais

<Obrigatório. Listar o que permanece sem validação nesta versão,
referenciando os IDs de docs/testing/test-matrix.md §11.>

## Limitações da execução

<O que este ambiente específico não conseguiu exercitar. Exemplo:
"Executado sem acesso à rede externa — o que não afeta a suíte, já que
nenhum teste depende de rede.">

## Conclusão técnica

<Critérios objetivos. Nomear o que foi validado e o que não foi.>

## Critério de liberação

- [ ] 118 testes passando
- [ ] `fmt --check` sem diferenças
- [ ] `clippy -D warnings` sem apontamentos
- [ ] Nenhum snapshot pendente (`*.snap.new`)
- [ ] `sqlx prepare --check` sem divergência
- [ ] CSS compilado em dia
- [ ] `cargo audit` sem advisory novo não justificado
- [ ] Build de release sem warnings
- [ ] Riscos residuais registrados e aceitos

**Decisão:** liberado | liberado com ressalvas | bloqueado
````

## 4. Exemplo de conclusão adequada

**Inadequado** — não é verificável e não diz o que ficou de fora:

> A versão aparenta estar estável. Todos os testes passaram e o sistema parece
> funcionar corretamente.

**Adequado** — nomeia o validado, o não validado e a base da decisão:

> A versão atende aos critérios definidos para os testes unitários, de integração com
> banco real e de contrato executados: 118 testes, sem falhas, contra PostgreSQL 18.
> As verificações de formatação, lint, frescor do cache SQLx e do CSS compilado
> passaram sem divergência.
>
> Permanecem **sem validação automatizada**: reversão das 11 migrações (N-01),
> comportamento de JavaScript em navegador (N-04), reconciliação entre `holdings` e
> `transactions` (N-05), corrida nas primitivas de concorrência dos jobs (N-06) e
> comportamento sob carga (N-09). Nenhum destes é regressão desta versão — são
> lacunas permanentes documentadas em `test-matrix.md` §11.
>
> A cobertura instrumentada continua **não medida** (DT-08).
>
> **Decisão: liberado**, com os riscos residuais acima registrados e aceitos.

## 5. Coleta dos dados do relatório

```bash
git rev-parse HEAD
```

```bash
cargo --version && rustc --version
```

```bash
cargo test 2>&1 | tail -20
```

```bash
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings
```

Verificar se há snapshot pendente:

```bash
find . -name '*.snap.new' -not -path './target/*'
```

## 6. Evidências

```text
- .github/workflows/ci.yml       (os comandos que o CI executa)
- docs/testing/test-plan.md §6   (critérios de entrada, saída e bloqueio)
- docs/testing/test-matrix.md §11 (riscos sem cobertura, com IDs)
- docs/testing/coverage.md       (estado da medição)
```
