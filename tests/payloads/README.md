# Payloads

Respostas **reais** das APIs de terceiros de que o serviço depende, capturadas
do endpoint de produção e versionadas aqui.

## Por que payload real, e não fixture escrito à mão

Um fixture inventado testa a nossa ideia do formato. O formato é do outro lado.
As duas coisas divergem exatamente onde dói:

- A Coinbase entrega cada taxa como **string**, não como número — e com
  precisão arbitrária. Em `coinbase_exchange_rates.json` a maior taxa
  (`OOKI`) tem **41 dígitos significativos**, mais que os 28 da mantissa do
  `Decimal`. Um fixture com `"BTC": 0.0000031` teria passado por anos e nunca
  contado que o mapa inteiro é decodificado de uma vez: uma taxa que não caiba
  derruba a resposta toda, e com ela a sincronização de **todos** os pares.
- A CoinGecko manda `null` em campos que o tipo declara como número (moeda
  recém-listada sem série de 24 h, sem oferta divulgada) e um `roi` que às
  vezes é objeto, às vezes `null`. São 30 campos por moeda, dos quais lemos 15.

Por isso os testes de contrato (`tests/payload_market.rs`,
`tests/payload_quotes.rs`) atravessam as **mesmas funções que o servidor
atravessa** — `market::parse_markets` e `quotes::parse_brl_rates` — com estes
arquivos como entrada. Se a fonte renomear um campo, trocar um tipo ou estourar
uma escala, o teste quebra aqui, em CI, e não silenciosamente na tela.

## Arquivos

| Arquivo | Origem | Consumido por |
|---|---|---|
| `coingecko_markets.json` | `GET api.coingecko.com/api/v3/coins/markets?vs_currency=brl&order=market_cap_desc&per_page=4&page=1&locale=pt&sparkline=true&price_change_percentage=1h,24h,7d` | `market::parse_markets` → tela `/market` |
| `coinbase_exchange_rates.json` | `GET api.coinbase.com/v2/exchange-rates?currency=BRL` | `quotes::parse_brl_rates` → preços de `assets.unit_value` |

Capturados em **2026-07-29**. `per_page=4` no primeiro: o formato de uma linha é
idêntico ao das 100 que a produção pede, e quatro moedas já cobrem os casos que
importam (com e sem `roi`, com e sem campo nulo) sem versionar 1 MB de série
temporal.

## Como recapturar

O `User-Agent` não é decoração: a CoinGecko responde **403** a requisição sem
ele (o `reqwest` não manda nenhum por padrão — foi assim que o feed falhou na
primeira rodada real).

```bash
curl -sS -A "wallet/0.1.0" "https://api.coinbase.com/v2/exchange-rates?currency=BRL" | python -m json.tool > tests/payloads/coinbase_exchange_rates.json
```

Recapturar muda os números, não o formato — e é o formato que os testes
verificam. Nenhuma asserção depende da cotação do dia: elas conferem invariantes
(escala travada, campo obrigatório presente, ausente virando neutro), nunca
"BTC vale R$ 327.777". Um teste que precisasse de recaptura semanal seria um
alarme falso semanal.
