# ADR-0006: sqlx com verificação em compilação e cache offline versionado

## Status

Aceita.

## Contexto

O acesso a dados precisava atender a três exigências simultâneas, e nenhuma delas é
negociável num sistema financeiro:

1. **SQL explícito e auditável.** Num sistema onde uma query errada move dinheiro,
   o autor precisa ver exatamente o SQL executado — não uma DSL que gera SQL.
2. **Erro de schema detectado antes de produção.** Coluna renomeada, tipo trocado,
   `NULL` não tratado: tudo isso é erro de runtime na maioria das abordagens.
3. **Compilar sem banco de dados.** O lint do CI, o build Docker e o
   rust-analyzer na máquina do desenvolvedor não devem exigir Postgres de pé. Sem
   isso, todo `query_as!` vira erro de compilação quando o banco está desligado —
   e o rust-analyzer passa a acusar o arquivo inteiro em vermelho (`os error
   10061`).

O material do curso apresentou também Diesel como alternativa (módulo M16), o que
torna esta uma escolha deliberada, não default.

## Restrições

- Rust + tokio ([ADR-0001](0001-rust-como-linguagem-unica.md),
  [ADR-0002](0002-axum-em-vez-de-rocket.md)) — o acesso a dados precisa ser async.
- PostgreSQL com `NUMERIC` de precisão arbitrária
  ([ADR-0004](0004-decimal-e-numeric-para-dinheiro.md)) e integração direta com
  `rust_decimal`.
- Recursos de SQL que o projeto **depende** e que abstrações costumam esconder:
  `FOR UPDATE`, `UPDATE ... RETURNING`, `ON CONFLICT`, chave primária composta,
  `ROUND` sobre agregado.
- A suíte de testes precisa de isolamento real por teste, com migrações aplicadas.

## Opções consideradas

**Avaliadas de fato** (comparação documentada em
[../aprendizado/08-persistencia-e-orm.md](../aprendizado/08-persistencia-e-orm.md)):

1. **sqlx** com macros verificadas em compilação — decisão adotada.
2. **Diesel** — ORM com DSL própria, apresentado no curso.
3. **Armazenamento em JSON/arquivo** — apresentado no curso e descartado por
   ausência de transação e constraint.

**Comparação *post hoc***: SeaORM, `tokio-postgres` puro.

## Decisão

**sqlx 0.9** com:

- `sqlx::query!` / `query_as!` — queries verificadas contra o schema real em tempo
  de compilação.
- **Cache offline `.sqlx/` versionado** (31 arquivos de query) e
  `SQLX_OFFLINE = "true"` em `.cargo/config.toml`.
- `sqlx::migrate!()` — migrações embutidas no binário e aplicadas no boot.
- `#[sqlx::test]` — banco efêmero por teste, migrado automaticamente.

Consultas montadas dinamicamente ficam **restritas a dois lugares**: o bootstrap do
catálogo e o `SELECT 1` da sonda de readiness.

## Fundamentação

**Motivo confirmado** para preferir SQL explícito a ORM: o material do curso
discute a alternativa, e o projeto manteve todo o SQL visível em um único arquivo
(`src/repository.rs`, 24 métodos públicos).

**Justificativa técnica** do ganho concreto: a verificação em compilação transforma
uma classe inteira de erro de runtime em erro de build. Isso é a **mesma disciplina**
que o Askama aplica a templates ([ADR-0003](0003-ssr-com-askama-e-htmx.md)) e o
`i18n::Strings` aplica a traduções — e essa coerência é um dos traços mais
consistentes do projeto. Três verificações estáticas diferentes, o mesmo princípio:
falhar no build, não em produção.

**Motivo confirmado** para o cache offline versionado — o comentário em
`.cargo/config.toml` é explícito:

> "Compila contra o cache `.sqlx/` versionado em vez de abrir conexão com o
> Postgres. Sem isso, todo `query_as!` vira erro de compilação quando o banco não
> está de pé — inclusive no rust-analyzer, que passa a acusar o arquivo inteiro em
> vermelho (`os error 10061`)."

E há um detalhe de implementação registrado: entradas de `[env]` no config do cargo
**não** sobrescrevem variáveis já definidas no ambiente, então `cargo sqlx prepare`
(que exporta `SQLX_OFFLINE=false`) continua regenerando o cache normalmente.

**Contrapartida assumida e mitigada.** Cache versionado pode descolar do schema. O
CI roda `cargo sqlx prepare --check` no job `test` justamente para provar que não
descolou — o mesmo padrão aplicado ao CSS compilado
([ADR-0010](0010-css-compilado-em-build-time.md)).

**`#[sqlx::test]` como escolha de isolamento.** Cria um banco **efêmero por teste**,
com migrações aplicadas, e o derruba ao fim. Consequência: os 26 testes do
repository são isolados e paralelos, nenhum depende da ordem nem do estado deixado
por outro. Um dublê de banco não teria nenhuma das garantias que esses testes
existem para verificar — transação, `FOR UPDATE`, `CHECK`, `UNIQUE`, `NUMERIC`.

## Consequências positivas

- Coluna renomeada ou tipo trocado quebram o build, não a produção.
- SQL visível e auditável — relevante onde a query move dinheiro.
- `lint` do CI e build Docker dispensam Postgres.
- rust-analyzer funciona com o banco desligado.
- Migrações embutidas no binário: o deploy dispensa passo manual.
- 26 testes isolados e paralelos contra Postgres real.
- Acesso direto a `FOR UPDATE` e `UPDATE ... RETURNING`, que sustentam a atomicidade
  das operações e a rotação de sessão sem corrida.

## Consequências negativas

- **Passo manual a cada query nova:** `cargo sqlx prepare` precisa ser executado, e
  esquecer disso quebra o CI (o que é o comportamento desejado, mas é atrito).
- **31 arquivos JSON versionados** que ninguém lê, poluindo o diff.
- **Consulta dinâmica perde a verificação** — o projeto restringe a dois lugares,
  mas a restrição é disciplina, não constraint.
- **Renomear coluna é caro:** migração + regeneração de cache + ajuste de código.
- **A suíte completa exige Docker.** `cargo test` sem Postgres falha na maior parte
  dos testes; só `payload_market` e `payload_quotes` rodam isolados.
- Migração automática no boot significa que um deploy com migração ruim **derruba o
  serviço** em vez de subir degradado — deliberado, mas é uma consequência real.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Cache `.sqlx/` descolar do schema | Médio — build passa localmente e falha no CI, ou pior, compila contra schema errado | `cargo sqlx prepare --check` no CI |
| Migração falha no boot em produção | **Alto** — serviço não sobe | Deliberado ("melhor não subir que subir contra schema pela metade"); **mas não há teste que aplique e reverta as migrações** |
| Query dinâmica introduzida sem verificação | Médio | Restrição por convenção; revisão de código |
| sqlx quebrar API em versão menor | Médio | Versão fixada; `Cargo.lock` versionado |

## Evidências

```text
- Cargo.toml                 (sqlx 0.9.0; features macros, postgres,
                              runtime-tokio, migrate, time, rust_decimal)
- src/repository.rs          (query!/query_as! em todos os 24 métodos)
- .sqlx/                     (31 queries em cache, versionadas)
- .cargo/config.toml         (SQLX_OFFLINE = "true", com o motivo comentado)
- .github/workflows/ci.yml   (cargo sqlx migrate run; cargo sqlx prepare --check)
- src/app.rs                 · AppState::build (sqlx::migrate!().run)
- Dockerfile                 (ENV SQLX_OFFLINE=true no estágio builder)
- migrations/                (11 pares up/down)
```

## Critérios de revisão

Reavaliar se:

1. O projeto passar a precisar de **query dinâmica extensa** — filtros compostos
   opcionais, ordenação escolhida pelo usuário. Aí o ponto forte do sqlx deixa de
   se aplicar à maior parte do código, e um query builder passa a valer.
2. O atrito de `cargo sqlx prepare` custar mais do que a verificação rende (sinal:
   CI quebrando por cache desatualizado com frequência).
3. Surgir necessidade de suportar mais de um banco.

**Recomendação não implementada:** um teste que aplique todas as migrações e as
reverta em sequência, fechando o risco "migração falha no boot". Hoje a
reversibilidade dos 11 arquivos `.down.sql` é afirmada por construção, **nunca
verificada**.
