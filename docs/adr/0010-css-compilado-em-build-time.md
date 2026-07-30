# ADR-0010: CSS compilado em build-time pelo CLI standalone, sem Node

## Status

Aceita. **Substitui** a decisão anterior de usar o Tailwind Play CDN, que foi
revogada.

## Contexto

A interface usava o **Tailwind Play CDN**. Isso significa, concretamente, que o
navegador baixava um **compilador de CSS de 407 KB em JavaScript** a cada
carregamento de página, e esse compilador injetava `<style>` no DOM em runtime.

Duas consequências, e a primeira é a que decidiu a questão:

1. **A CSP não podia fechar.** Estilo injetado em runtime exige `style-src
   'unsafe-inline'` — a diretiva que mais enfraquece uma política, porque reabre a
   porta para XSS via atributo de estilo. Enquanto o CDN estivesse lá, a CSP seria
   decorativa nesse ponto.
2. **407 KB de JavaScript por carregamento**, mais uma requisição a terceiro,
   numa aplicação cujo custo de JS deveria ser próximo de zero
   ([ADR-0003](0003-ssr-com-askama-e-htmx.md)).

Havia ainda uma restrição de contexto: o projeto **não tem Node nem npm** em nenhum
ponto do build, e não queria adquiri-los.

## Restrições

- CSP fechada em `'self'`, sem `'unsafe-inline'`, como meta
  ([ADR-0003](0003-ssr-com-askama-e-htmx.md)).
- Nenhuma dependência do ecossistema JavaScript na cadeia de build.
- O artefato de deploy é um binário único: o CSS precisa ser embutível.
- O build precisa ser **determinístico** entre plataformas — o CI roda Linux, o
  desenvolvimento é Windows.
- Zero requisições a terceiros em runtime.

## Opções consideradas

**Avaliadas de fato:**

1. **Tailwind Play CDN** — estado anterior, **revogado**.
2. **Tailwind via npm/PostCSS** — traria Node e a árvore npm ao build.
3. **CLI standalone do Tailwind** (executável único) — decisão adotada.

**Comparação *post hoc***: CSS artesanal (sem utilitários, sem ferramenta de build);
Sass/SCSS (outra ferramenta externa, sem o ganho de utilitários).

## Decisão

`styles/app.css` (3,8 KB, fonte) é compilado para `static/app.css` (19 KB,
minificado) pelo **executável standalone do Tailwind**, e o resultado é
**versionado** no repositório — pelo mesmo critério do cache `.sqlx/`
([ADR-0006](0006-sqlx-com-checagem-em-compilacao.md)).

O binário embute `static/app.css` via `include_str!` e o serve em `/static/app.css`.

A varredura automática de conteúdo é **desligada** (`source(none)`), com as fontes
declaradas explicitamente (`@source "../templates/**/*.html"`).

O CI recompila e faz `diff` para provar que o artefato versionado está em dia.

## Fundamentação

**Motivo confirmado** para sair do CDN — o comentário de topo em `styles/app.css` é
explícito:

> "Por que build-time e não o Play CDN: o CDN é um COMPILADOR que roda no navegador
> (407 KB de JS por carregamento) e injeta `<style>` em runtime — o que obrigava a
> CSP a carregar `style-src 'unsafe-inline'`, justamente a diretiva que mais
> enfraquece uma política. Com o CSS estático, a CSP fecha e o custo em JS cai a
> zero."

**Motivo confirmado** para o CLI standalone em vez de npm: é um executável único —
sem Node, sem npm, **sem herdar a cadeia de suprimentos do ecossistema JavaScript**.
Este é o traço mais distintivo da decisão: o projeto mantém **zero dependências JS
na cadeia de build**, e o único JavaScript que existe (htmx, `money-input.js`) é
código pronto, vendorado, servido pelo próprio binário.

**Motivo confirmado, e não óbvio, para `source(none)`.** O comentário registra um
problema descoberto na prática:

> "`source(none)` DESLIGA a varredura automática do projeto — sem isso o Tailwind
> vasculha o repositório inteiro e recolhe candidatos de arquivos que não são
> template (inclusive do próprio CSS gerado, que assim se auto-alimenta e nunca
> solta uma classe removida). O efeito prático era um build não-determinístico: a
> mesma entrada rendia CSS diferente em máquinas diferentes, e o check de frescor do
> CI quebrava sozinho."

Ou seja: sem `source(none)`, o gerador lê o próprio output, o CSS nunca encolhe
quando uma classe é removida, e o `diff` do CI falha de forma intermitente e
inexplicável. Com a fonte declarada, **o CSS é função apenas dos templates** —
reprodutível em qualquer plataforma.

**Por que versionar o artefato compilado.** O binário o embute, então ele precisa
existir no repositório para o build funcionar. O risco de descolar dos templates é
real e recebeu a mesma mitigação do cache `.sqlx`: um passo de CI que recompila e
compara. O comentário no workflow nomeia a consequência de não ter isso: "Classe
nova usada sem recompilar = estilo faltando em produção, e nada no build de Rust
perceberia."

Há um caso concreto registrado de cache cego causando problema: o teste
`static_assets_revalidate_by_content_and_answer_304_when_unchanged` existe porque "a
URL é fixa e o conteúdo muda a cada build: com cache cego, o rebuild deixa HTML novo
com CSS velho. Foi o que empilhou o painel de mercado."

**Paleta validada por script.** Os valores de cor em `styles/app.css` foram
verificados contra a superfície escura por um validador (banda de luminosidade, piso
de croma, separação sob daltonismo, contraste), não escolhidos a olho. O achado que
moldou o desenho: verde e vermelho ficam a **ΔE ~4,6 sob deuteranopia** —
indistinguíveis para cerca de 8% dos homens. Consequência de projeto: **nenhuma
variação é comunicada só por cor**; toda variação sai com seta ▲/▼ **e** sinal, e a
cor é canal redundante. O acento é violeta justamente para não disputar hue com o par
lucro/prejuízo.

## Consequências positivas

- CSP fecha em `'self'`, sem `'unsafe-inline'`.
- 407 KB de JavaScript eliminados do carregamento.
- Zero requisições a terceiros para estilo.
- Zero dependências npm na cadeia de build.
- Build determinístico entre plataformas.
- CSS de 19 KB minificado, embutido no binário.
- Classe removida do HTML desaparece do arquivo gerado.
- Acessibilidade a daltonismo garantida por redundância de canal, travada por teste
  (`market_dashboard_marks_direction_with_arrow_and_sign_not_only_colour`).

## Consequências negativas

- **O CSS compilado é artefato versionado e pode descolar dos templates.** O CI
  prova o frescor, mas recompilar é passo manual a cada classe nova.
- **O binário do CLI não é versionado** (`/tools/` está no `.gitignore`): quem
  precisa recompilar baixa a versão certa por conta própria.
- **A versão está fixada em dois lugares diferentes** — o CI baixa `v4.3.3`
  explicitamente, e o desenvolvedor local pode ter outra, gerando `diff` espúrio.
  **Este é o defeito mais concreto da decisão** e está registrado como débito
  técnico.
- **Classes montadas dinamicamente não são detectadas** pelo gerador com
  `source(none)`.
- O `diff` do CSS gerado polui o histórico de commits.
- O passo de CI depende de baixar um binário do GitHub Releases — dependência de
  rede externa no pipeline.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| CSS versionado descolar dos templates | Médio — estilo faltando em produção, invisível ao build de Rust | Job `lint` do CI recompila e faz `diff` |
| Divergência de versão do CLI (local vs CI) | Médio — `diff` espúrio, falso negativo | **Nenhuma.** Registrado como débito |
| Cache de navegador servindo CSS velho | Baixo (já ocorreu) | `ETag` de conteúdo + `no-cache` + 304; dois testes travam |
| GitHub Releases indisponível no CI | Baixo | Nenhuma |

## Evidências

```text
- styles/app.css              (fonte; o porquê do build-time e do source(none))
- static/app.css              (19 KB minificado, versionado, embutido)
- .gitignore                  (/tools/ — o CLI não é versionado)
- .github/workflows/ci.yml    (job lint: baixa v4.3.3, recompila, diff)
- src/routes/frontend.rs      · app_css (include_str! + ETag)
- src/app.rs                  · security_headers (CSP sem unsafe-inline)
- testes: pages_carry_no_inline_style_or_script,
          static_assets_revalidate_by_content_and_answer_304_when_unchanged,
          static_assets_revalidate_instead_of_being_cached_blind,
          market_dashboard_marks_direction_with_arrow_and_sign_not_only_colour
```

## Critérios de revisão

Reavaliar se:

1. A divergência de versão do CLI produzir falsos negativos com frequência — aí
   fixar a versão num arquivo lido pelo CI **e** por um script local resolve sem
   mudar a decisão de fundo.
2. O projeto adquirir Node por outro motivo — aí o CLI standalone perde a sua
   principal vantagem.
3. O Tailwind mudar o modelo de distribuição do executável standalone.
4. A quantidade de CSS artesanal crescer a ponto de os utilitários deixarem de
   compensar a ferramenta.
