# Payloads e contratos de dados

## Objetivo

Documentar campo a campo cada payload que entra ou sai do sistema: tipo,
obrigatoriedade, restrições, origem do valor, quem consome, validações aplicadas,
sensibilidade e riscos de compatibilidade.

## Escopo

Coberto: os 4 payloads de entrada da API/formulários, o payload de saída da API, o
formato do CSV e os 2 payloads de integrações externas. Não coberto: rotas e códigos
de resposta (ver [endpoints.md](endpoints.md)) e o schema do banco (ver
[../data/database-schema.md](../data/database-schema.md)).

> **Todos os valores de exemplo neste documento são fictícios.** Nenhuma credencial,
> token, chave ou cotação real aparece aqui. Cotações são ilustrativas e não devem
> ser lidas como preço de mercado.

---

## 1. Convenção que atravessa todos os payloads

**Dinheiro é sempre string em JSON, nunca número.**

```json
{ "unit_value": "327777.41000000" }
```

Não é preferência de formatação. Um número JSON seria decodificado como `f64` por
qualquer cliente padrão, e o erro de representação binária que isso introduz é
exatamente o que o sistema evita de ponta a ponta
([ADR-0004](../adr/0004-decimal-e-numeric-para-dinheiro.md)). O teste
`the_catalogue_round_trips_through_real_http_requests` verifica essa propriedade
explicitamente.

Na **entrada**, o `rust_decimal` aceita tanto `"750.25"` (string) quanto `750.25`
(número JSON) — mas a forma recomendada para um cliente é string, porque preserva a
precisão em toda a cadeia. Escala máxima aceita: **8 casas** (`MONEY_SCALE`).

---

## 2. Payloads de entrada — API JSON

### 2.1 `CreateAssetRequest` — `POST /api/v1/assets`

```json
{
  "name": "ouro",
  "unit_value": "750.25"
}
```

| Campo | Tipo | Obrigatório | Restrições | Descrição |
| --- | --- | :---: | --- | --- |
| `name` | string | **Sim** | Não vazio após `trim`; `UNIQUE` no banco | Nome do ativo no catálogo |
| `unit_value` | decimal (string) | **Sim** | `>= 0`; escala ≤ 8 | Preço unitário inicial em BRL |

**Por que cada campo existe.** `name` é a chave de casamento da sincronização de
cotações, que compara por nome normalizado (`LOWER(TRIM(...))`) — por isso o nome é
trimado na escrita: espaço invisível criaria um ativo duplicado que a sincronização
nunca atualizaria. `unit_value` é **o preço que lastreia compra e venda**; é o campo
mais sensível da API inteira.

**Origem do valor:** operador administrativo ou script de integração.
**Consumidores:** `repository.create_asset` → tabela `assets` → toda operação
financeira.

**Validações, na ordem:**

1. `validated_asset_name` — rejeita vazio ou só espaços ⇒ `InvalidAssetName` (`400`).
2. `validated_unit_value` — rejeita negativo ⇒ `NegativeUnitValue` (`400`); arredonda
   para `MONEY_SCALE`.
3. `CHECK (unit_value >= 0)` no schema — última linha de defesa.

**Exemplos válidos:**

```json
{ "name": "ouro",         "unit_value": "750.25" }
{ "name": "prata",        "unit_value": "0" }
{ "name": "  platina  ",  "unit_value": "9.87654321" }
```

O terceiro grava `"platina"`, já trimado.

**Exemplos inválidos:**

| Corpo | Resultado |
| --- | --- |
| `{ "name": "", "unit_value": "10" }` | `400` — nome vazio |
| `{ "name": "   ", "unit_value": "10" }` | `400` — só espaços |
| `{ "name": "ouro", "unit_value": "-1" }` | `400` — preço negativo |
| `{ "name": "ouro" }` | `400` — campo ausente |
| `{ "name": 123, "unit_value": "10" }` | `400` — tipo trocado |
| `{ "name": "ouro", "unit_value": "10"` | `400` — JSON malformado |
| `{ "name": "ouro", "unit_value": "1.123456789" }` | Aceito, **arredondado** para 8 casas |
| `{ "name": "bitcoin", ... }` (já existente) | **`500`** — ver aviso abaixo |

> **Divergência conhecida.** Nome duplicado viola o `UNIQUE` de `assets.name` e
> **não** tem tratamento dedicado: vira `AppError::Database` ⇒ `500`. Contraste com
> `users.username`, que é traduzido para `UsernameTaken` ⇒ `400`. Registrado como
> **DT-06** em [../decisions/technical-debt.md](../decisions/technical-debt.md).

**Dados sensíveis:** nenhum. **Retenção:** permanente enquanto o ativo existir.

### 2.2 `UpdateAssetRequest` — `PATCH /api/v1/assets`

```json
{
  "id": 1,
  "unit_value": "760.10"
}
```

| Campo | Tipo | Obrigatório | Restrições | Descrição |
| --- | --- | :---: | --- | --- |
| `id` | inteiro (i64) | **Sim** | Deve existir | Identificador do ativo |
| `name` | string | Não | Não vazio se presente | Novo nome |
| `unit_value` | decimal (string) | Não | `>= 0`; escala ≤ 8 | Novo preço |

Atualização **parcial**: campos ausentes preservam o valor atual (via `COALESCE` no
SQL). Enviar só `id` é válido e não altera nada.

`id` inexistente ⇒ `404` explícito. Entrada inválida ⇒ `400` **com o ativo
intocado** — travado por `asset_update_rejects_invalid_input`.

**Idempotente:** o mesmo corpo produz o mesmo resultado.

**Risco de compatibilidade:** acrescentar um campo opcional é retrocompatível.
Tornar `name` obrigatório, ou mudar o tipo de `id`, quebraria consumidores — e é o
tipo de mudança que exigiria `/api/v2`
([ADR-0011](../adr/0011-versionamento-da-api-por-caminho.md)).

## 3. Payloads de entrada — formulários HTML

`Content-Type: application/x-www-form-urlencoded`.

### 3.1 `LoginForm` — `POST /login` e `POST /register`

| Campo | Tipo | Obrigatório | Restrições | Sensível |
| --- | --- | :---: | --- | :---: |
| `username` | string | **Sim** | 3–32 caracteres (só no cadastro); trimado | Não |
| `password` | string | **Sim** | 8–128 caracteres (só no cadastro) | **Sim** |
| `csrf_token` | string | **Sim** | Deve bater com o cookie `csrf` | Sim |

**`password` nunca é armazenada nem registrada em log.** Os handlers usam
`#[instrument(skip_all)]` precisamente para que os argumentos não entrem no span. Só
a hash argon2 chega ao banco, gerada em `UnauthenticatedUser::register` — o
`Repository` nunca vê texto livre.

O limite superior de 128 caracteres existe porque um campo sem teto é vetor de
negação de serviço no cálculo da hash, que é deliberadamente custoso.

### 3.2 `AmountForm` — `POST /deposit`

| Campo | Tipo | Obrigatório | Restrições | Descrição |
| --- | --- | :---: | --- | --- |
| `amount` | decimal | **Sim** | `> 0`; escala ≤ 8 | Quantia a creditar em BRL |
| `csrf_token` | string | **Sim** | Deve bater com o cookie | — |

Zero e negativo são recusados (`InvalidAmount` ⇒ `400`) — "depósito negativo é saque
disfarçado". Escala acima de 8 casas é recusada **antes** de tocar o banco, mas zeros
à direita não contam: `1.010` é aceito como `1.01`, porque `1.010 == 1.01` não pode
virar erro.

> O campo é `<input type="number">` de verdade. A máscara monetária
> (`money-input.js`) é **aditiva**: sem JavaScript o campo continua funcionando, e
> **toda** a validação de valor permanece no servidor.

### 3.3 `TradeAssetForm` — `POST /buy` e `POST /sell`

| Campo | Tipo | Obrigatório | Restrições | Descrição |
| --- | --- | :---: | --- | --- |
| `asset_id` | inteiro (i64) | **Sim** | Deve existir no catálogo | Ativo negociado |
| `quantity` | decimal | **Sim** | `> 0`; escala ≤ 8 | Unidades a negociar |
| `csrf_token` | string | **Sim** | Deve bater com o cookie | — |

**O preço não vem do formulário.** É lido de `assets.unit_value` dentro da transação,
no momento da operação. Aceitar preço do cliente permitiria comprar ao preço que o
cliente escolhesse.

Recusas específicas, todas com reversão completa da transação:

| Situação | Erro | Motivo |
| --- | --- | --- |
| Saldo insuficiente | `InsufficientBalance` (`400`) | Compra parcialmente aplicada é pior que recusada |
| Posição insuficiente | `InsufficientHoldings` (`400`) | Venda a descoberto não existe neste produto |
| Ativo sem cotação (preço 0) | `QuoteUnavailable` (`502`) | Negociar a zero seria transferência de valor |
| Total arredonda a zero | `TradeTooSmall` (`400`) | Senão viraria um moedor de unidades grátis |

A interface só oferece ativos **negociáveis** no formulário de compra — oferecer um
ativo sem preço levaria o usuário a um erro que ele não causou.

### 3.4 `SyncQuotesForm` — `POST /quotes/sync`

| Campo | Tipo | Obrigatório |
| --- | --- | :---: |
| `csrf_token` | string | **Sim** |

Cooldown de 30 s para chamadas manuais ⇒ `QuoteSyncTooSoon` (`429`).

## 4. Payload de saída — `Asset`

Único tipo serializado pela API.

```json
{
  "id": 1,
  "name": "bitcoin",
  "unit_value": "327777.41000000"
}
```

| Campo | Tipo | Sempre presente | Origem | Consumidor |
| --- | --- | :---: | --- | --- |
| `id` | inteiro | Sim | `assets.id` (`BIGSERIAL`) | Referência em `PATCH` e nos formulários |
| `name` | string | Sim | `assets.name` | Exibição e casamento da sincronização |
| `unit_value` | **string** decimal | Sim | `assets.unit_value` (`NUMERIC`) | Preço de compra/venda |

O formato é **congelado por snapshot** (`insta`): mudar qualquer campo exige
`cargo insta review` explícito. Os três snapshots versionados estão em
`src/routes/snapshots/`. O snapshot de criação registra literalmente
`"unit_value": "10"` — confirmando a serialização como string.

**Não expõe** nenhum dado de usuário. `password_hash`, `balance` e `role` nunca são
serializados: `UserRecord` sequer deriva `Serialize`.

## 5. Formato do CSV do extrato

`GET /transactions.csv` — `text/csv; charset=utf-8`, com
`Content-Disposition: attachment; filename="extrato.csv"`.

```csv
data;tipo;ativo;quantidade;preco_unitario;movimento_caixa
2026-07-30 14:22;Compra;bitcoin;0,50000000;327777,41000000;-163888,70500000
2026-07-29 09:15;Depósito;;;;1000,00
```

| Coluna | Origem | Vazia quando |
| --- | --- | --- |
| `data` | `transactions.created_at`, `AAAA-MM-DD HH:MM` | Nunca |
| `tipo` | `transactions.kind`, traduzido | Nunca |
| `ativo` | `assets.name` via `JOIN` | Depósito |
| `quantidade` | `transactions.quantity` | Depósito |
| `preco_unitario` | `transactions.unit_value` | Depósito |
| `movimento_caixa` | `transactions.cash_delta` (assinado) | Nunca |

Convenções, todas deliberadas para planilha em pt-BR:

- **Separador `;`** — é o que Excel/LibreOffice em português leem como coluna.
- **Decimal com vírgula.**
- **Aspas internas dobradas** (RFC 4180).
- **Tipo traduzido** conforme o idioma da interface.

Contém o histórico financeiro completo do usuário: **é dado sensível**. A resposta
usa `Cache-Control: no-store` e exige sessão.

## 6. Integração externa — Coinbase (lastreia dinheiro)

```text
GET https://api.coinbase.com/v2/exchange-rates?currency=BRL
```

| Aspecto | Valor |
| --- | --- |
| Autenticação | Nenhuma (API pública) |
| `User-Agent` | `wallet/<versão>` |
| Timeout | 15 s |
| Tentativas | Nenhuma — a próxima rodada do job é a retentativa |
| Frequência | `QUOTES_SYNC_MINUTES` (padrão 10 min) |
| Consumidor | `quotes::parse_brl_rates` → `assets.unit_value` |

Estrutura da resposta (valores fictícios):

```json
{
  "data": {
    "currency": "BRL",
    "rates": {
      "USD": "0.1950439818938865",
      "EUR": "0.1702276222772165",
      "BTC": "0.00000305062380408311740629700305"
    }
  }
}
```

| Campo | Tipo | Uso |
| --- | --- | --- |
| `data.currency` | string | **Não lido** — a URL já fixa BRL |
| `data.rates` | objeto `{ moeda: string }` | **O único campo consumido** |

### As três propriedades perigosas desta integração

Verificadas na captura real versionada em `tests/payloads/coinbase_exchange_rates.json`:

1. **As taxas são string com precisão arbitrária.** A captura tem **636 taxas**, e a
   maior (`OOKI`) tem **41 dígitos significativos** — contra os 28 da mantissa do
   `Decimal`. Verificado.

2. **O mapa é decodificado de uma vez.** Uma única taxa que não coubesse derrubaria a
   resposta inteira, e com ela a sincronização de **todos** os pares. O sintoma seria
   "os preços pararam", sem nada apontando a causa.

3. **O preço é o inverso da taxa.** `BRL→USD = 0,195...` significa `1 USD ≈ 5,13
   BRL`. A inversão **precisa** ser arredondada para `MONEY_SCALE`: a divisão de
   `Decimal` preenche a mantissa inteira, e foi exatamente isso que causou o
   incidente de 500 em `/assets`.

Pares consumidos (`MARKET_PAIRS`), com os aliases que casam no catálogo:

| Código | Nome canônico | Aliases |
| --- | --- | --- |
| `USD` | dólar | `dolar`, `dólar`, `usd` |
| `EUR` | euro | `euro`, `eur` |
| `BTC` | bitcoin | `bitcoin`, `btc` |
| `ETH` | ethereum | `ethereum`, `eth` |
| `SOL` | solana | `solana`, `sol` |

Além desses, `real` e `brl` são injetados com preço fixo `1` — a moeda de
denominação não precisa de cotação. **Nenhuma documentação anterior mencionava esses
dois** (divergência D5 do diagnóstico).

Degradação: par ausente da resposta é **pulado** sem afetar os outros; taxa `<= 0`
vira `None`; corpo malformado vira `AppError::Payload` ⇒ `502`, nunca pânico.

## 7. Integração externa — CoinGecko (informativa)

```text
GET https://api.coingecko.com/api/v3/coins/markets
    ?vs_currency=brl&order=market_cap_desc&per_page=100&page=1
    &locale=pt&sparkline=true&price_change_percentage=1h,24h,7d
```

| Aspecto | Valor |
| --- | --- |
| Autenticação | Nenhuma |
| `User-Agent` | `wallet/<versão>` — **obrigatório** |
| Timeout | 15 s |
| Frequência | `MARKET_SYNC_SECONDS` (padrão 60 s) |
| Consumidor | `market::parse_markets` → snapshot **em memória** |
| Persistido? | **Não** |

> **A fonte responde `403` a requisição sem `User-Agent`**, e o `reqwest` não manda
> nenhum por padrão. A mesma URL respondia `200` no navegador e `403` no serviço —
> descoberto na primeira rodada real.

São **30 campos por moeda** na resposta; o sistema lê **16**:

| Campo lido | Tipo | Ausente ⇒ | Destino |
| --- | --- | --- | --- |
| `id` | string | — | Chave de seleção da tela |
| `symbol` | string | — | Ticker (normalizado para maiúsculas) |
| `name` | string | — | Exibição e busca |
| `current_price` | número | **Moeda descartada** | Cotação |
| `market_cap_rank` | número | 0 | Ordenação |
| `price_change_percentage_1h_in_currency` | número | 0 neutro | Variação 1 h |
| `price_change_percentage_24h` | número | 0 neutro | Variação 24 h |
| `price_change_percentage_7d_in_currency` | número | 0 neutro | Variação 7 d |
| `market_cap` | número | 0 neutro | Capitalização |
| `total_volume` | número | 0 neutro | Volume 24 h |
| `high_24h` / `low_24h` | número | 0 neutro | Faixa do dia |
| `ath` / `ath_change_percentage` | número | 0 neutro | Máxima histórica |
| `circulating_supply` | número | 0 neutro | Oferta |
| `sparkline_in_7d.price` | array de número | Sem gráfico | Série temporal |

**A política de degradação é assimétrica, e é deliberada:** sem preço (ou com `NaN`
ou `inf`) a moeda é **descartada** — preço é o único campo indispensável. Qualquer
outro campo ausente vira **zero neutro** e a moeda **permanece**, porque "uma linha
útil pelo preço é melhor que uma linha a menos".

**Escalas travadas na fronteira:** `MONEY_SCALE` (8) para preços, `CHANGE_SCALE` (2)
para variações, `AGGREGATE_SCALE` (2) para agregados. A conversão usa `round_dp`
porque `from_f64_retain` traz o erro de representação binária (0,1 vira
0,1000000000000000055…).

**Armadilhas do formato real**, verificadas na captura:

- `roi` é às vezes objeto, às vezes `null` — não é lido, mas quebraria um tipo
  ingênuo.
- Campos numéricos vêm `null` para moedas recém-listadas.
- **A série `sparkline_in_7d.price` não tem tamanho fixo.** A captura real traz
  **167** amostras para bitcoin/ethereum/tether e **169** para binancecoin, embora a
  documentação interna descreva "168 amostras" como valor nominal. O código trata
  isso corretamente (`Range::window` não estoura em série curta), e há teste para
  série curta — mas **o número não deve ser tratado como garantido**.

## 8. O que nunca aparece em payload

| Dado | Onde vive | Por que nunca sai |
| --- | --- | --- |
| Senha em texto | Em lugar nenhum | Só a hash argon2 é persistida |
| `password_hash` | `users.password_hash` | `UserRecord` não deriva `Serialize` |
| Refresh token em claro | Cookie do navegador | No banco vai só a hash SHA-256 |
| `ADMIN_SECRET_KEY` | Ambiente | Nunca serializado nem logado |
| `JWT_SECRET` | Ambiente | Idem |
| `DATABASE_URL` | Ambiente | Erros 5xx são censurados antes da resposta |
| Detalhe de erro interno | Log do servidor | `IntoResponse` substitui por `"internal server error"` |

O conteúdo do JWT **é legível** por quem tem o cookie — ele é assinado, não
criptografado. Carrega `id`, `username` e `role`; nada além disso deve ser colocado
lá.

## 9. Evidências

```text
- src/routes/api.rs      · CreateAssetRequest, UpdateAssetRequest
- src/routes/frontend.rs · LoginForm, AmountForm, TradeAssetForm, SyncQuotesForm,
                           LangQuery, MarketQuery, PageQuery, transactions_to_csv
- src/models.rs          · Asset, MONEY_SCALE
- src/quotes.rs          · CoinbaseRatesResponse, CoinbaseRates, parse_brl_rates,
                           brl_price, MARKET_PAIRS, RATES_URL
- src/market.rs          · MarketRow, Series, parse_markets, decimal_from_f64,
                           MARKETS_URL, USER_AGENT
- src/repository.rs      · validated_asset_name, validated_unit_value
- src/routes/snapshots/  (3 snapshots de contrato)
- tests/payloads/        (capturas reais + README)
- tests/payload_quotes.rs, tests/payload_market.rs (12 testes de contrato)
```
