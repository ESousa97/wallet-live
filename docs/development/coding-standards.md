# Padrões de código

## Objetivo

Documentar as convenções de código **efetivamente praticadas** no projeto, as regras
que o ferramental impõe automaticamente e os invariantes que dependem de disciplina
humana.

## Escopo

Coberto: formatação, nomenclatura, comentários, tratamento de erro, regras de camada e
os invariantes que nenhuma ferramenta verifica. Não coberto: arquitetura (ver
[../architecture/component-architecture.md](../architecture/component-architecture.md))
e commits (ver [commit-conventions.md](commit-conventions.md)).

---

## 1. O que o ferramental garante

| Ferramenta | Regra | Onde |
| --- | --- | --- |
| `rustfmt` | Formatação padrão, sem configuração customizada | `cargo fmt --all --check` |
| `clippy` | **Warnings são erro** | `cargo clippy --all-targets -- -D warnings` |
| `sqlx` | Queries verificadas contra o schema | Compilação |
| `askama` | Variáveis de template verificadas | Compilação |
| `i18n::Strings` | Texto faltando num idioma | Compilação |
| Compilador | Exaustividade de `match`, tipos, ownership | Compilação |

**Não há `rustfmt.toml` nem `clippy.toml`.** O projeto usa os padrões, deliberadamente:
menos configuração para manter, e qualquer pessoa com Rust instalado produz o mesmo
resultado.

## 2. Nomenclatura

Segue as convenções do Rust, com uma característica própria:

| Elemento | Convenção | Exemplo |
| --- | --- | --- |
| Módulo, função, variável | `snake_case` | `wallet_summary`, `brl_price` |
| Tipo, trait, enum | `PascalCase` | `AppError`, `PortfolioRepository` |
| Constante | `SCREAMING_SNAKE_CASE` | `MONEY_SCALE`, `FREE_ATTEMPTS` |
| **Testes** | Frase descritiva | `buy_rejects_when_balance_is_insufficient` |

### Nomes de teste são frases

É o traço de nomenclatura mais distintivo do projeto. O nome do teste **enuncia o
invariante**, não o método testado:

```text
buying_more_averages_the_cost_basis
selling_everything_closes_the_position
legacy_high_scale_money_still_renders_the_wallet
forms_without_a_matching_csrf_token_are_refused
the_unversioned_alias_serves_the_same_thing_as_v1
```

A vantagem prática: uma falha em `sell_rejects_more_than_owned` já diz o que quebrou,
sem abrir o arquivo.

> **Alguns testes estão em português** (`coin_normaliza_simbolo_e_trava_a_escala`,
> `medidor_da_faixa_de_24h_fica_dentro_do_viewbox`), enquanto a maioria está em inglês.
> É uma inconsistência real. **Prefira inglês** para novos testes, por ser a maioria e
> por acompanhar o idioma dos identificadores do código.

## 3. Comentários

O projeto tem **densidade alta e deliberada** de comentários, e há um padrão claro
sobre o que eles registram.

### A regra: comentário explica o *porquê*, não o *quê*

O código já diz o que faz. O comentário registra a decisão — e é dessa prática que
grande parte desta documentação foi reconstruída.

**Padrão praticado:**

```rust
/// Escala canônica de dinheiro/preço: TODA gravação monetária arredonda para
/// até 8 casas decimais. O invariante existe porque `NUMERIC` do Postgres é
/// ilimitado, mas `rust_decimal::Decimal` tem 28 dígitos significativos:
/// valores de escala alta tornam PRODUTOS e SOMAS no SQL indecodificáveis na
/// leitura (`value not representable`) — derrubando a tela da carteira.
pub const MONEY_SCALE: u32 = 8;
```

O comentário explica o mecanismo da falha e a consequência concreta. Um comentário que
dissesse "escala de 8 casas" seria inútil.

### O que merece comentário

| Situação | Exemplo real |
| --- | --- |
| Decisão não óbvia | Por que `RwLock` no mercado e `Mutex` nas cotações |
| Armadilha descoberta na prática | A CoinGecko responde 403 sem `User-Agent` |
| Ordem que importa | A ordem das `.layer()` no router |
| Invariante frágil | `MONEY_SCALE` e os `ROUND` nos agregados |
| Escolha entre alternativas | Por que SHA-256 e não argon2 no refresh token |
| Comportamento que surpreende | Por que `Admin` nega sem cair no header |

### O que **não** merece comentário

Código autoexplicativo, repetição da assinatura, ou comentário que envelhece com o
código (números de linha, contagens que mudam).

### Módulos com documentação de topo

Os módulos com decisão de fundo carregam `//!` explicando a natureza do componente.
`market.rs` é o melhor exemplo: sua primeira frase estabelece que **aquele feed não
move dinheiro** — a informação mais importante sobre o módulo.

## 4. Tratamento de erro

| Regra | Prática |
| --- | --- |
| Um erro para o sistema | `AppError`, 21 variantes |
| Propagação | `?`, com `#[from]` do `thiserror` |
| Erro de negócio | Variante **tipada** própria, mapeada para 4xx |
| Erro interno | `Database`/`Template` ⇒ 5xx **censurado** |
| `unwrap`/`expect` | **Só** onde a falha é impossível por construção, com mensagem explicativa |
| `panic!` | Uma ocorrência deliberada: hash de senha ilegível |

### A regra que sustenta as camadas

**O repositório não reinterpreta erro, e o serviço tampouco.** Três testes travam
isso (`deposit_result_flows_through_unchanged` e equivalentes), porque uma camada que
"melhora" o erro da camada abaixo esconde a causa.

### `expect` aceitável

```rust
.expect("cliente HTTP de cotações")   // falha só se a configuração do builder for inválida
.expect("static format")              // formato literal, validado na compilação
.expect("non-empty")                  // precedido de verificação de tamanho
```

A mensagem descreve **a premissa**, não o erro.

## 5. Regras de camada

| Regra | Verificável em |
| --- | --- |
| `repository` não conhece HTTP | Sem `axum::http` nos `use` |
| `repository` não conhece `services`/`routes` | Dependência unidirecional |
| `services` depende de trait, não de tipo concreto | `PortfolioService<R: PortfolioRepository>` |
| `market` não escreve no banco | Sem `Repository` nos `use` |
| Todo SQL vive em `repository.rs` | Um único arquivo |
| Handlers são finos | Sem regra de negócio em `routes/` |

Detalhamento em
[../architecture/component-architecture.md](../architecture/component-architecture.md) §1.

## 6. Invariantes que **nenhuma ferramenta verifica**

A parte mais importante deste documento. Estas regras dependem de disciplina humana, e
violá-las não quebra o build:

### 6.1 Dinheiro nunca é ponto flutuante

`f64` é permitido **exclusivamente** em coordenadas de desenho SVG. Qualquer valor
monetário é `Decimal` ↔ `NUMERIC`.

Ao converter de fonte externa que entrega `f64`, **trave a escala na fronteira**
(`decimal_from_f64`): `from_f64_retain` traz o erro de representação binária.

### 6.2 Toda escrita monetária arredonda para `MONEY_SCALE`

```rust
let total = (price * quantity).round_dp(MONEY_SCALE);
```

Nada no compilador força isso. O `CHECK` do schema pega valor **negativo**, não escala
excessiva.

### 6.3 Todo agregado SQL de dinheiro usa `ROUND(..., 8)`

**A regra mais frágil do projeto.** Produtos e somas de `NUMERIC` acumulam escala sem
limite:

```sql
SELECT ROUND(SUM(h.quantity * a.unit_value), 8) AS holdings_value
```

Omitir o `ROUND` numa query nova reproduz o incidente de 2026-07-22 — e o teste de
regressão existente **não pega esse caso**, porque ele verifica o estado antigo do
banco, não uma query nova.

### 6.4 Nenhum `<style>` ou `<script>` inline

É o que permite a CSP fechar em `'self'`. **Este** tem teste
(`pages_carry_no_inline_style_or_script`).

Consequência: indicador proporcional é **geometria de SVG**, não `style="width:63%"`.

### 6.5 Direção nunca é comunicada só por cor

Verde e vermelho ficam a **ΔE ~4,6 sob deuteranopia**. Toda variação sai com seta ▲/▼
**e** sinal. Travado por teste para o painel de mercado; **não** para telas novas.

### 6.6 Erro de negócio não vira 5xx

Saldo insuficiente é `400` com mensagem real, não `500` censurado.

### 6.7 Handler não registra argumento sensível

`#[instrument(skip_all)]` em todo handler que recebe formulário. Sem isso, senha e
valores entram no span.

## 7. Padrões recorrentes

### Extrator como injeção de dependência

```rust
async fn handler(
    State(state): State<AppState>,
    SessionUser(user): SessionUser,   // exige sessão
    portfolio: PortfolioService,      // injeta o serviço
    locale: Locale,                   // idioma resolvido
) -> Result<Response, AppError>
```

A assinatura declara o que o handler exige. **A proteção de uma rota é visível na sua
assinatura.**

### Tipo como prova

`User` tem campos privados: só se obtém passando por um fluxo de autenticação. `Admin`
é uma unit struct cujo único conteúdo é a prova de autorização.

Ao criar um extrator novo: **a construção deve ser a verificação**.

### Validação na borda da escrita

`validated_asset_name`, `validated_unit_value` — funções privadas chamadas por todo
caminho de escrita, antes de tocar o banco.

### Consultas concorrentes

```rust
let (a, b, c) = tokio::try_join!(f1(), f2(), f3())?;
```

Para consultas **independentes**. O tempo total é o da mais lenta.

## 8. Testes: convenções

| Regra | Prática |
| --- | --- |
| Unidade | `#[cfg(test)] mod tests` no mesmo arquivo |
| Contrato | `tests/*.rs` |
| Com banco | `#[sqlx::test]` — banco efêmero por teste |
| Nome | Frase que enuncia o invariante |
| Documentação | Toda entrada nova vai para [../testing/test-catalogue.md](../testing/test-catalogue.md) |
| Caminho de dinheiro | **Postgres real**, nunca dublê |

> Um teste cujo motivo não está escrito é um teste que alguém apaga no primeiro
> refactor em que ele incomoda.

## 9. Lista de verificação antes de commitar

| # | Item | Comando |
| --- | --- | --- |
| 1 | Formatação | `cargo fmt --all` |
| 2 | Lint | `cargo clippy --all-targets -- -D warnings` |
| 3 | Testes | `cargo test` |
| 4 | Query nova | `cargo sqlx prepare` |
| 5 | Classe CSS nova | Recompilar `static/app.css` (v4.3.3) |
| 6 | Contrato JSON alterado | `cargo insta review` |
| 7 | **Escrita monetária nova** | Tem `round_dp(MONEY_SCALE)`? |
| 8 | **Agregado SQL novo** | Tem `ROUND(..., 8)`? |
| 9 | Template novo | Sem `<style>`/`<script>` inline |
| 10 | Handler novo com formulário | Tem `#[instrument(skip_all)]`? |
| 11 | Rota nova | A proteção está na assinatura? |
| 12 | Teste novo | Foi ao catálogo? |
| 13 | Decisão não óbvia | Está comentada com o porquê? |

Os itens **7 e 8** são os que nenhuma ferramenta verifica e os que já causaram
incidente.

## 10. Evidências

```text
- src/models.rs          · MONEY_SCALE (padrão de comentário)
- src/market.rs          (doc de módulo //! estabelecendo a natureza)
- src/repository.rs      · validated_*, wallet_summary (ROUND)
- src/auth/user.rs       · User (campos privados)
- src/auth/admin.rs      · Admin (unit struct como prova)
- src/error.rs           · AppError
- src/services/portfolio.rs · PortfolioService (genérico sobre trait)
- .github/workflows/ci.yml  (fmt, clippy -D warnings)
```
