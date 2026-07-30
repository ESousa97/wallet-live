# ADR-0009: Snapshot de mercado em memória, fora do banco

## Status

Aceita.

## Contexto

A tela de mercado exibe as 100 maiores criptomoedas em BRL, com cotação, variações
de 1 h/24 h/7 d, capitalização, volume, faixa de negociação do dia, máxima histórica
e uma série temporal para o gráfico.

Havia três problemas a resolver simultaneamente:

1. **Onde guardar esse dado.** A opção natural — persistir no banco — colocaria
   cotação informativa de terceiro na mesma base que o catálogo que lastreia
   operações reais.
2. **Quantas chamadas externas fazer.** Se cada requisição do usuário chamasse a
   API, o consumo do limite gratuito seria proporcional ao tráfego, e trocar de
   moeda na tela custaria uma chamada.
3. **Concorrência.** Muitas leituras simultâneas (requisições HTTP) contra uma
   escrita periódica (o job).

## Restrições

- A CoinGecko é API pública sem chave, com limite de taxa e cache de ~60 s do
  próprio lado — pedir mais rápido não traz número novo.
- A CoinGecko devolve **número JSON**, que o `serde_json` decodifica como `f64` —
  precisão suficiente para exibir, insuficiente para contabilizar
  ([ADR-0004](0004-decimal-e-numeric-para-dinheiro.md)).
- O projeto tem instância única, sem cache distribuído.
- A CSP fecha `style-src` em `'self'`
  ([ADR-0003](0003-ssr-com-askama-e-htmx.md)), então indicadores proporcionais não
  podem usar `style="width:63%"`.

## Opções consideradas

**Avaliadas de fato:**

1. **Persistir o snapshot no banco** — rejeitada.
2. **Chamar a API a cada requisição do usuário** — rejeitada.
3. **Snapshot em memória, atualizado por job periódico** — decisão adotada.

**Comparação *post hoc***: cache externo (Redis) — resolveria o compartilhamento
entre réplicas, ao custo de uma dependência de infraestrutura que o projeto não tem.

## Decisão

O snapshot vive **exclusivamente em memória**, num `Arc<Market>` com
`RwLock<Snapshot>` dentro do `AppState`. Um job atualiza a cada
`MARKET_SYNC_SECONDS` (padrão 60, zero desliga). **A requisição do usuário apenas
lê** — nunca chama a API externa.

A série temporal de 7 dias vem **no mesmo pedido** (`sparkline=true`, 168 amostras
por moeda), e a janela de 24 h é o rabo dessa série.

## Fundamentação

**Motivo confirmado**, e o comentário em `AppState` é explícito sobre o principal:

> "Fica fora do banco de propósito: é dado de terceiro, volátil e puramente
> informativo — perder no restart não custa nada, e gravar misturaria cotação de
> fora com o catálogo que lastreia as operações."

A separação entre as **duas** integrações externas é a decisão de fundo, e vale
enunciá-la explicitamente:

| | Coinbase (`quotes.rs`) | CoinGecko (`market.rs`) |
| --- | --- | --- |
| Papel | **Lastreia dinheiro**: define `assets.unit_value` | **Informativo**: só a tela de mercado |
| Formato do número | **String** de precisão arbitrária → `Decimal` sem passar por float | Número JSON → `f64` → `Decimal` com escala travada |
| Persistido | Sim, em `assets` | **Não** |
| Concorrência | `Mutex` (exclusão mútua na escrita) | `RwLock` (leituras concorrentes) |

**Este feed não move dinheiro.** O módulo declara isso na primeira linha da sua
documentação. Se as duas fontes fossem misturadas, um `f64` da CoinGecko poderia
acabar lastreando uma compra — exatamente o que
[ADR-0004](0004-decimal-e-numeric-para-dinheiro.md) existe para impedir.

**`RwLock` e não `Mutex`, e a diferença é justificada:** toda requisição HTTP **lê**
o snapshot, só o job **escreve** (uma vez por minuto). `RwLock` permite leituras
concorrentes sem fila — o encaixe certo quando leitura domina esmagadoramente. O job
de cotações usa `Mutex` justamente porque lá o caso é o oposto: duas escritas
simultâneas no catálogo seriam um problema real.

**Um snapshot serve todas as interações.** Trocar de moeda, trocar a janela do
gráfico ou buscar na lista **não custa nenhuma chamada externa** — a tela responde
igual com um ou mil acessos, e o limite da API não depende do tráfego. É o ganho
mais concreto da decisão.

**Degradação definida em detalhe.** A política de campo ausente é assimétrica e
deliberada:

- **Sem preço, ou preço `NaN`/`inf`** → a moeda é **descartada**. Preço é o único
  campo indispensável.
- **Qualquer outro campo ausente** → vira **zero neutro** e a moeda **permanece** na
  lista. O comentário justifica: "uma linha útil pelo preço é melhor que uma linha a
  menos".
- **Snapshot vazio** (antes da primeira rodada) → a tela mostra `role="status"` em
  vez de quebrar.

**Detalhe de integração descoberto na prática, e registrado:** a CoinGecko responde
**403 a requisição sem `User-Agent`**, e o `reqwest` não manda nenhum por padrão. A
mesma URL respondia 200 no navegador e no PowerShell (que mandam UA) e 403 no
serviço. Sem isso, o feed nunca sobe.

**Escalas travadas na fronteira:** `MONEY_SCALE` (8) para preços, `CHANGE_SCALE` (2)
para variações, `AGGREGATE_SCALE` (2) para agregados. `decimal_from_f64` usa
`round_dp` porque `from_f64_retain` traz o erro de representação binária.

**Indicadores como geometria de SVG.** `trading_range_x` devolve uma **coordenada de
`viewBox`**, não uma porcentagem — porque a CSP proíbe `style` inline. E aplica
`clamp`, porque "a fonte publica preço, mínima e máxima em apurações diferentes; um
preço fora da faixa por alguns centavos acontece e não pode empurrar o marcador para
fora do medidor."

## Consequências positivas

- Cotação informativa nunca contamina o catálogo financeiro.
- Consumo da API externa é constante (1 chamada/minuto), independente do tráfego.
- Trocar de moeda ou de janela é instantâneo e gratuito.
- Leituras concorrentes sem fila.
- Nenhuma migração, nenhum crescimento de tabela, nenhuma retenção a gerenciar.
- Tela degrada com dignidade antes da primeira rodada e quando a fonte falha.
- 11 testes de unidade + 7 de contrato contra payload real cobrem o parse e a
  projeção.

## Consequências negativas

- **O snapshot não sobrevive ao restart.** Após cada deploy, a tela de mercado fica
  em estado de carregamento por até `MARKET_SYNC_SECONDS`.
- **Não há histórico.** A série temporal é a que a fonte entrega; o sistema não
  acumula série própria. Se a CoinGecko parar de mandar `sparkline`, o gráfico
  desaparece — e o teste
  `the_payload_carries_the_time_series_that_makes_the_chart_free` existe para que
  isso não passe em silêncio.
- **Não é compartilhável entre réplicas.** Cada instância manteria o seu snapshot e
  faria as suas chamadas — multiplicando o consumo do limite da API pelo número de
  réplicas.
- **Memória proporcional ao payload.** 100 moedas × 168 amostras de série, mantidas
  vivas continuamente.
- **`f64` na série temporal**, deliberado (é coordenada de desenho), mas é uma
  exceção à regra do projeto que precisa estar documentada para não parecer descuido.
- **Nenhum teste de corrida** sobre o `RwLock`.
- Se a fonte ficar indisponível por muito tempo, a tela exibe dado defasado —
  há indicação de defasagem, mas o dado velho continua visível.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| CoinGecko mudar formato ou remover campo | Médio — tela degrada ou esvazia | 7 testes de contrato contra payload real versionado |
| Limite de taxa excedido | Baixo — 1 chamada/minuto | Intervalo alinhado ao cache da fonte (~60 s) |
| `f64` da série escapando para cálculo financeiro | **Alto** se acontecer | `series` é `pub(crate)`; só o projetor do gráfico a lê; documentado no tipo |
| Múltiplas réplicas multiplicando chamadas | Médio | **Nenhuma.** Registrado como limitação |
| Fonte responder 403 por falta de UA | Alto (já ocorreu) | `USER_AGENT` explícito no cliente, com o motivo comentado |

## Evidências

```text
- src/app.rs        · AppState::market (o porquê de ficar fora do banco)
- src/market.rs     · Market (RwLock), Coin, MarketRow::into_coin,
                      parse_markets, spawn_scheduled_refresh,
                      trading_range_x, decimal_from_f64,
                      MARKETS_URL, USER_AGENT, CHANGE_SCALE, AGGREGATE_SCALE
- src/config.rs     · market_sync_seconds
- tests/payloads/coingecko_markets.json  (payload real versionado)
- tests/payload_market.rs                (7 testes de contrato)
- testes de unidade em src/market.rs     (11)
- testes: the_market_screen_degrades_gracefully_before_the_first_refresh,
          the_market_screen_accepts_any_state_in_the_query_string,
          moeda_sem_preco_ou_com_preco_invalido_e_descartada,
          variacao_ausente_vira_zero_e_a_moeda_permanece
```

## Critérios de revisão

Reavaliar se:

1. O sistema ganhar **mais de uma instância** — aí um cache compartilhado (Redis)
   ou um serviço dedicado de cotação passa a valer, para não multiplicar o consumo
   da API.
2. Surgir requisito de **histórico próprio** de cotação (comparar preço de hoje com
   o de seis meses atrás) — aí persistir passa a ser necessário, mas em tabela
   **separada** do catálogo, para preservar a separação que este ADR estabelece.
3. A janela de indisponibilidade após restart se tornar inaceitável — mitigável com
   persistência de aquecimento, sem abandonar a decisão de fundo.
4. A CoinGecko passar a exigir chave de API ou mudar o modelo de limite.
