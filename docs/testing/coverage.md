# Cobertura de testes

## Objetivo

Registrar o estado real da medição de cobertura no projeto — que é a **ausência** de
qualquer ferramenta — e propor uma opção compatível com a linguagem e o pipeline
atuais.

## Escopo

Coberto: o estado atual, a proposta de instrumentação, o que a métrica significaria e
o que ela **não** significaria. Não coberto: quais testes existem (ver
[test-catalogue.md](test-catalogue.md)) e quais riscos estão descobertos (ver
[test-matrix.md](test-matrix.md) §11).

---

## 1. Estado atual: não há medição

**Nenhuma ferramenta de cobertura está configurada.** Não há `cargo-llvm-cov`,
`cargo-tarpaulin`, `grcov` nem instrumentação de qualquer tipo no `Cargo.toml` ou no
workflow de CI.

Consequência direta e importante:

> **O percentual de cobertura deste projeto é desconhecido.** Qualquer número citado
> em outro lugar seria inventado.

O que se sabe é qualitativo, não quantitativo:

| Sabe-se | Não se sabe |
| --- | --- |
| Que existem 118 testes | Quantas linhas eles exercitam |
| Quais riscos cada teste reduz (ver a matriz) | Quais ramos nunca são executados |
| Que módulos críticos têm teste | Se algum caminho de erro nunca é exercitado |
| Que `config.rs` **não** tem teste direto | O tamanho real da lacuna |

## 2. Por que a ausência é registrada em vez de resolvida

O projeto priorizou **cobertura por risco** sobre cobertura por linha: a
[matriz de testes](test-matrix.md) mapeia 60 riscos técnicos nomeados aos testes que
os reduzem, e lista explicitamente 10 riscos **sem** cobertura.

Isso é mais informativo que um percentual — mas **não substitui** a medição, porque
responde a uma pergunta diferente:

| Pergunta | Respondida por |
| --- | --- |
| "Os riscos que identifiquei estão cobertos?" | A matriz de testes |
| "Existe código que nenhum teste executa?" | **Ninguém hoje** |

A segunda pergunta é a que encontra risco **não identificado** — e é exatamente por
isso que a ausência de ferramenta é um débito, não uma decisão arquitetural.

Registrado como **DT-08** em
[../decisions/technical-debt.md](../decisions/technical-debt.md).

## 3. Proposta: `cargo-llvm-cov`

É a opção compatível com este projeto, e a recomendação é essa por três motivos
verificáveis:

| Critério | `cargo-llvm-cov` | `cargo-tarpaulin` |
| --- | --- | --- |
| Instrumentação | Nativa do LLVM, via `rustc` | `ptrace`, mais frágil |
| Plataformas | Linux, macOS, **Windows** | Linux principalmente |
| Testes de integração (`tests/`) | Suportados | Suporte irregular |
| Cobertura de ramo | Sim | Limitada |
| Integração com CI | `--lcov`, `--codecov`, `--json` | Sim |

O suporte a Windows importa aqui: o desenvolvimento é feito em Windows 11, e uma
ferramenta que só rodasse no CI não ajudaria durante a escrita do teste.

### Instalação

```bash
cargo install cargo-llvm-cov --locked
```

### Uso local

Com o Postgres de pé (a maioria dos testes precisa dele):

```bash
cargo llvm-cov --all-features --workspace --html
```

O relatório fica em `target/llvm-cov/html/index.html`.

Só o que não toca banco:

```bash
cargo llvm-cov --test payload_market --test payload_quotes --html
```

### Integração no CI

Acrescentar ao job `test` de `.github/workflows/ci.yml`, **depois** do passo de
testes:

```yaml
      - name: instala cargo-llvm-cov
        run: cargo install cargo-llvm-cov --locked
      - name: cobertura
        run: cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
```

> **Recomendação deliberada: não estabelecer um limiar de reprovação inicialmente.**
> Um piso arbitrário (`--fail-under-lines 80`) incentiva testes escritos para mover o
> número, que é o oposto do que este projeto pratica — todo teste aqui existe porque
> reduz um risco nomeado. Meça primeiro, entenda a distribuição, e só então considere
> um piso **por módulo crítico**.

## 4. O que medir, e por módulo

Cobertura agregada esconde justamente o que importa. A leitura útil é por módulo,
ponderada pela consequência de uma falha:

| Módulo | Consequência de falha | Expectativa |
| --- | --- | --- |
| `repository.rs` | **Valor errado no patrimônio** | Muito alta — todo caminho de escrita e toda guarda |
| `auth/*` | **Acesso indevido** | Muito alta — todo mecanismo de defesa |
| `quotes.rs` | Preço errado lastreando operações | Alta |
| `services/portfolio.rs` | Tela errada, dado correto | Alta |
| `models.rs` | Cálculo derivado errado | Alta |
| `error.rs` | Status HTTP errado, vazamento em 5xx | Média-alta |
| `routes/*` | Comportamento de interface | Média |
| `market.rs` | Tela informativa incorreta | Média |
| `i18n.rs` | Texto errado | Baixa |
| `config.rs` | **Serviço sobe sem segredo validado** | **Alta — e hoje é zero** |
| `app.rs` | Boot ou camadas quebrados | Média — parcialmente exercitado de forma indireta |

## 5. Tipos de cobertura, e o que cada um revelaria aqui

| Tipo | O que mede | Valor neste projeto |
| --- | --- | --- |
| **Linha** | Linhas executadas | Base; esconde ramos |
| **Função** | Funções chamadas ao menos uma vez | Encontraria código morto |
| **Ramo** | Cada lado de cada decisão | **O mais valioso aqui** — o sistema tem muitos caminhos de recusa (saldo, posição, escala, CSRF, lockout) que só existem para o caso negativo |
| **Condição** | Cada subexpressão booleana | Marginal — as condições são simples |

**Ramo é o que importa.** Um `if saldo < total { rollback }` com cobertura de linha
completa pode ter o ramo de recusa nunca executado — e é o ramo de recusa que protege
o dinheiro.

## 6. Exclusões esperadas

Código que **não** deveria contar contra a métrica:

| Código | Motivo |
| --- | --- |
| `#[cfg(test)] mod tests` | É o teste, não o alvo |
| `src/main.rs` | 8 linhas; chama `App::start` |
| `shutdown_signal` | Depende de sinal do SO |
| `init_otel`, `init_tracing` | Instalam estado global do processo |
| `impl Drop for OtelGuard` | Roda no fim do processo |
| `panic!` de hash ilegível | Caminho de estado impossível por construção |
| `EquityChart::empty` (só em teste) | Já marcado como `pub(crate)` |

Em `cargo-llvm-cov`, exclusões se declaram com `#[coverage(off)]` ou via
`--ignore-filename-regex`.

## 7. Código difícil ou inadequado de testar no nível atual

Registro honesto, para que a lacuna não seja lida como descuido:

| Código | Por que é difícil | Alternativa |
| --- | --- | --- |
| `fetch_brl_rates`, `market::fetch` | Fazem I/O de rede real | **Já separados** de `parse_*`, que são testados com payload real |
| `spawn_scheduled_*` | Laço infinito com `tokio::interval` | Testar `QuoteSync::run` isolado |
| `init_otel` | Instala provider global do processo | Verificação manual com o coletor local |
| `shutdown_signal` | Espera sinal do SO | Verificação manual |
| `Config::from_env` | Lê variáveis globais do processo | **Testável** com uma função pura que receba um mapa — refactor recomendado |
| Middleware isolado | Exige montar a pilha | **Já coberto** indiretamente pelos testes de contrato |

> `Config::from_env` é o caso mais acionável: extrair a validação para uma função que
> receba os valores em vez de lê-los do ambiente tornaria N-03 testável sem
> manipular variáveis globais.

## 8. O que a cobertura **não** significa

Registrado para que o número, quando existir, não seja mal interpretado:

1. **Cobertura alta não é qualidade.** Um teste que executa uma função sem verificar
   o resultado conta como cobertura e não protege nada.
2. **Cobertura de 100% não prova ausência de defeito.** A matriz lista 10 riscos
   descobertos — reversão de migração, corrida nos jobs, comportamento no DOM — que
   **nenhuma** métrica de linha revelaria.
3. **Cobertura não mede o que não existe.** Um caminho de erro que nunca foi
   implementado não aparece como lacuna.
4. **Cobertura ignora o valor do que é coberto.** Uma linha de `i18n.rs` conta igual
   a uma linha de `buy_asset`.
5. **Perseguir o número corrompe a suíte.** É por isso que a recomendação é medir sem
   limiar de reprovação inicial.

## 9. Recomendações, em ordem

1. **Instalar `cargo-llvm-cov` e medir uma vez**, localmente, para conhecer a
   distribuição real.
2. **Analisar por módulo**, começando por `repository.rs` e `auth/*`, com foco em
   **cobertura de ramo**.
3. **Refatorar `Config::from_env`** para receber os valores, tornando N-03 testável.
4. **Acrescentar o passo ao CI**, publicando o relatório como artefato — sem limiar.
5. **Considerar limiar por módulo crítico** só depois de duas ou três medições.

## 10. Evidências

```text
- Cargo.toml                 (nenhuma dependência de cobertura)
- .github/workflows/ci.yml   (nenhum passo de cobertura)
- src/config.rs              · Config::from_env (sem teste direto)
- docs/testing/test-matrix.md §11 (riscos sem cobertura)
```
