# ADR-0001: Rust como linguagem única, em um crate com alvo de biblioteca

## Status

Aceita.

## Contexto

O projeto é o trabalho final do bootcamp **Santander 2026 — Rust AI Developer**
(DIO). A linguagem, portanto, **não foi uma escolha livre**: era premissa do
contexto. Evidência: [../delivery/course-delivery.md](../delivery/course-delivery.md)
e [../aprendizado/README.md](../aprendizado/README.md).

Há, no entanto, uma decisão real dentro dessa premissa: **como estruturar o crate**.
O projeto começou com tudo em `src/main.rs`. Um binário Rust não pode ser importado
por um teste de integração — os arquivos em `tests/` são crates separados e só
alcançam o que uma **biblioteca** exporta. Enquanto o código vivia só no binário, a
única camada de teste possível era a de unidade, e nada exercitava o sistema
montado: middlewares, ordem de camadas, extratores, cookies.

## Restrições

- Linguagem fixada pelo contexto do curso.
- Autor único, sem equipe de manutenção.
- Sem infraestrutura de produção dedicada; o alvo é execução local reproduzível e
  container.
- Ambiente de desenvolvimento Windows, com restrições de rede corporativa (TLS
  interceptado, ausência de `cmake`) que afetam a escolha de dependências.

## Opções consideradas

**Avaliadas de fato** (há evidência de deliberação no repositório):

1. **Manter tudo em `src/main.rs`** — estado inicial do projeto.
2. **Adicionar `src/lib.rs` ao mesmo crate**, com `main.rs` reduzido a
   `tokio::main -> App::start()`.
3. Workspace com múltiplos crates (`wallet-core`, `wallet-web`).

**Comparação técnica *post hoc*** (não houve deliberação; registrada em
[../architecture/technology-decisions.md](../architecture/technology-decisions.md)
§1): TypeScript/Node, Go, Java/Kotlin, C#/.NET.

## Decisão

Rust como linguagem única de todo o sistema — backend, regras financeiras,
renderização de HTML e projeção de SVG —, num **único crate com dois alvos**:
biblioteca (`src/lib.rs`) e binário (`src/main.rs`, de 8 linhas).

## Fundamentação

**Motivo confirmado** para o alvo de biblioteca: habilitar a camada de testes de
contrato. O comentário em `Cargo.toml` e a documentação de testes nomeiam isso
diretamente — "a pasta `tests/` só existe porque o crate ganhou um alvo de
biblioteca". Hoje são **35 testes** que só existem por causa desta decisão,
incluindo os que atravessam o router de produção com todos os middlewares na ordem
real.

**Motivo confirmado** para recusar o workspace: não há necessidade. Nenhum dos
alvos é publicado separadamente, e a divisão interna por módulos (`routes` →
`services` → `repository`) já sustenta a separação de responsabilidades sem o custo
de coordenar versões entre crates.

**Justificativa técnica inferida** de por que a premissa se mostrou adequada ao
domínio financeiro:

- **Tipos que carregam invariantes.** `User` tem campos privados: a única forma de
  obtê-lo é passar por um fluxo de autenticação, então tê-lo em mãos é prova de
  autenticação cumprida. `Admin` é uma unit struct cujo único conteúdo é a prova de
  autorização.
- **Exaustividade.** O `match` que mapeia `AppError` (21 variantes) para status HTTP
  não compila se uma variante nova ficar sem tratamento.
- **Verificação em compilação estendida.** Combinada com sqlx e Askama, uma coluna
  renomeada ou uma variável de template ausente viram erro de build. É a mesma
  disciplina aplicada a SQL, HTML e traduções.

## Consequências positivas

- 35 testes de contrato passaram a ser possíveis, incluindo autorização real e
  cabeçalhos de segurança exercitados pela pilha completa.
- `main.rs` com 8 linhas: toda a inicialização é testável porque vive em `App`.
- Distribuição como binário único, sem runtime nem interpretador na imagem final.
- Ausência de segunda linguagem no servidor elimina uma classe de erro de fronteira
  (serialização entre linguagens, duplicação de validação).

## Consequências negativas

- **Curva de aprendizado real** para qualquer manutenção futura: ownership,
  lifetimes e async.
- **Tempo de compilação** na ordem de minutos em build limpo de release; o CI
  depende de cache para ser tolerável.
- **Ecossistema web menos maduro** em nichos. Efeito concreto: `jwt-simple` traz
  `rsa` transitivamente, com advisory sem correção upstream (ver
  [ADR-0007](0007-sessao-jwt-curto-com-refresh-rotativo.md)).
- **Verbosidade de fronteira**: cada extrator exige uma `impl FromRequestParts`.
- O alvo duplo exige atenção a visibilidade: itens usados pelos testes de contrato
  precisam ser `pub`, o que amplia a superfície pública além do estritamente
  necessário.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Manutenção futura sem familiaridade em Rust | Alto — projeto fica parado | Densidade alta de comentários explicativos no código; documentação em `docs/architecture/` e `docs/adr/` |
| Superfície pública ampliada pelo alvo de biblioteca | Baixo | Uso de `pub(crate)` onde possível (`User::new`, `from_auth_token`) |
| Dependência de crates de nicho abandonados | Médio | `cargo audit` no CI |

## Evidências

```text
- Cargo.toml               (edition 2024; um crate, dois alvos)
- src/main.rs              (8 linhas: tokio::main -> App::start)
- src/lib.rs               (35 linhas: declara os módulos como biblioteca)
- src/app.rs               · App::router (separado de start justamente para o teste)
- tests/common/mod.rs      (monta o app de produção sem abrir socket)
- src/auth/user.rs         · User (campos privados)
- src/error.rs             (enum de 21 variantes)
```

## Critérios de revisão

Reavaliar se:

1. O projeto ganhar um segundo binário (CLI administrativa, worker separado) que
   compartilhe o núcleo — aí um workspace passa a fazer sentido.
2. A manutenção passar para uma equipe sem familiaridade com Rust.
3. Um requisito exigir biblioteca que só exista madura em outro ecossistema.

A escolha da linguagem em si **não é revisável** sem reescrita completa; o registro
existe para que a origem (premissa de curso, não avaliação comparativa) fique
honesta.
