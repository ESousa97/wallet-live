# ADR-0003: SSR com Askama + htmx, sem SPA

## Status

Aceita. Substitui parcialmente a abordagem inicial de recarregar a página inteira
a cada operação.

## Contexto

A interface do produto é composta de formulários (depósito, compra, venda),
tabelas (posições, extrato) e painéis de leitura (mercado). Não há edição
colaborativa, arraste, canvas nem estado de cliente complexo.

Havia dois problemas concretos a resolver:

1. **Recarregar a página inteira a cada operação** era a experiência inicial. Um
   depósito recarregava toda a carteira, incluindo o gráfico e o extrato.
2. **A CSP não podia fechar.** Enquanto existisse CSS ou JavaScript inline, a
   política precisaria de `'unsafe-inline'` em `style-src` — a diretiva que mais
   enfraquece uma CSP, porque reabre a porta para XSS injetado em atributo de
   estilo.

## Restrições

- Rust no servidor ([ADR-0001](0001-rust-como-linguagem-unica.md)); nenhuma
  intenção de introduzir Node ou npm no build (ver
  [ADR-0010](0010-css-compilado-em-build-time.md)).
- A aplicação deve funcionar **sem JavaScript**: é um requisito de acessibilidade
  e de robustez, não uma preferência estética.
- Um único binário como artefato de deploy — sem servidor de assets separado.
- CSP fechada em `'self'` como meta.

## Opções consideradas

**Avaliadas de fato:**

1. **SSR puro com recarregamento completo** — estado inicial.
2. **SSR + htmx com fragmentos** — decisão adotada.
3. **SPA** (React/Vue/Svelte) consumindo uma API JSON.

**Comparação *post hoc***: Tera/Handlebars (templates em runtime), Maud (HTML em
macro Rust), Alpine.js.

## Decisão

Renderização no servidor com **Askama** (templates verificados em compilação), e
**htmx 2.0.8 vendorado no binário** adicionando troca de fragmento como
*progressive enhancement*.

Cada tela tem **duas structs** que compartilham o mesmo dado e o mesmo fragmento
interno: `AssetsPage`/`WalletFragment` e `MarketPage`/`MarketFragment`. O handler
escolhe entre elas conforme o header `HX-Request`.

## Fundamentação

**Motivo confirmado**, em três frentes.

**Askama, verificação em compilação.** As variáveis usadas no `.html` são checadas
contra a struct correspondente em tempo de build. É a mesma disciplina que sqlx
aplica ao SQL e o `i18n::Strings` às traduções — coerência deliberada, não
coincidência.

**htmx com dois caminhos simultâneos no mesmo HTML.** Toda ação carrega o
`action`/`method` de um formulário POST normal **e** os atributos `hx-*` que
interceptam o mesmo clique quando JavaScript está disponível. Não há dois códigos
de handler para o mesmo dado: `is_partial_request` lê o header e `render_wallet`
devolve fragmento ou página inteira. Sem JavaScript, o PRG clássico continua
inteiro.

Detalhe que só aparece na implementação: `HX-History-Restore-Request` recebe a
**página inteira**, não o fragmento — voltar/avançar com o cache expirado precisa
reconstruir o DOM do zero. Travado pelo teste
`htmx_marks_partial_requests_but_history_restore_wants_the_full_page`.

**A CSP que isso protege.** Nenhum `<script>` ou `<style>` inline existe em
nenhuma página. O teste `pages_carry_no_inline_style_or_script` itera todo
`<script` de cada página renderizada e falha se algum não tiver `src=`. Isso é o
que permite `script-src 'self'; style-src 'self'` sem `'unsafe-inline'`.

Consequência não óbvia: **todo indicador proporcional da interface é geometria de
SVG**, não CSS inline. Não existe `style="width:63%"` — o medidor da faixa de
negociação devolve uma coordenada de `viewBox` (`trading_range_x`), porque atributo
XML não é bloqueado pela política. O gráfico de patrimônio e o de mercado seguem a
mesma regra: o servidor calcula o `<path>` e o template só interpola string.

## Consequências positivas

- CSP fechada em `'self'`, sem `'unsafe-inline'` — verificado em cada resposta,
  inclusive de erro (`every_api_response_carries_the_security_headers`).
- Zero requisições a terceiros: htmx e CSS saem do próprio binário.
- Operações sem recarregar a página, mantendo o servidor dono do HTML.
- A aplicação funciona sem JavaScript, incluindo todas as operações financeiras.
- Um artefato de deploy; nenhum build de JavaScript, nenhuma cadeia npm.
- Erro de template é erro de compilação, não erro de renderização em produção.

## Consequências negativas

- **htmx não é testado no DOM.** A suíte verifica o HTML emitido — atributos
  `hx-*`, ordem dos `<script>`, presença de `defer` — não o comportamento em
  navegador. Um erro de runtime no htmx passaria. Registrado em
  [../decisions/known-limitations.md](../decisions/known-limitations.md).
- **Cada mudança de HTML exige recompilar** o binário.
- **Estado complexo de cliente exigiria repensar a abordagem** por inteiro.
- **htmx vendorado precisa de atualização manual**, e o `cargo audit` não o alcança
  — é JavaScript, não crate.
- A duplicação de structs por tela (página/fragmento) é uma pequena repetição
  estrutural, mitigada por compartilharem o tipo de dado e o fragmento.
- Ordem dos `<script>` importa: `money-input.js` usa `htmx.onLoad` para reanexar a
  máscara após cada troca de fragmento, então precisa vir **depois** do htmx, os
  dois com `defer`. Ordem invertida = máscara morta. Travado por teste.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Erro de runtime no htmx passa pela suíte | Médio — operação silenciosamente inerte para quem tem JS | O caminho sem JS continua funcional; nenhuma operação depende exclusivamente do htmx |
| CSS/JS inline reintroduzido por engano | Médio — estilo ignorado em produção, difícil de rastrear | Teste `pages_carry_no_inline_style_or_script` |
| htmx vendorado com vulnerabilidade não detectada | Médio | **Nenhuma automatizada.** Ver recomendação em [../architecture/technology-decisions.md](../architecture/technology-decisions.md) |
| Cache de asset servindo CSS velho com HTML novo | Baixo (já ocorreu) | `ETag` de conteúdo + `no-cache` + 304; dois testes travam |

## Evidências

```text
- Cargo.toml                    (askama 0.16.0, features = ["derive"])
- templates/                    (base, login, assets, wallet, market,
                                 market_dashboard, market_macros)
- static/htmx.js                (2.0.8, vendorado, servido via include_str!)
- static/money-input.js         (máscara aditiva; validação segue no servidor)
- src/routes/frontend.rs        · render_wallet, is_partial_request,
                                  app_css, htmx_js, money_input_js
- src/market.rs                 · trading_range_x (coordenada, não porcentagem CSS)
- src/services/portfolio.rs     · equity_chart (SVG projetado no servidor)
- src/app.rs                    · security_headers (CSP sem unsafe-inline)
- testes: pages_carry_no_inline_style_or_script,
          the_wallet_fragment_is_partial_html_embedded_by_the_full_page,
          htmx_marks_partial_requests_but_history_restore_wants_the_full_page,
          money_input_script_loads_after_htmx_with_defer
```

## Critérios de revisão

Reavaliar se:

1. A interface passar a exigir estado de cliente complexo (formulários com
   dependência dinâmica profunda, edição colaborativa).
2. Surgir requisito de atualização em tempo real via WebSocket ou SSE para muitos
   elementos simultâneos.
3. O htmx vendorado receber advisory relevante sem caminho de atualização.
4. A ausência de teste de DOM produzir um incidente real — aí o custo de um teste
   de navegador headless passa a se justificar.
