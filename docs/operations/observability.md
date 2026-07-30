# Observabilidade

## Objetivo

Documentar os sinais que o sistema emite — logs, traces, métricas e sondas —, como
correlacioná-los e o que **não** é observável hoje.

## Escopo

Coberto: os quatro sinais existentes, formato, correlação, configuração e limitações.
Não coberto: procedimentos de diagnóstico (ver [runbooks.md](runbooks.md)) e a
justificativa da arquitetura (ver
[../adr/0012-observabilidade-opt-in-via-otlp.md](../adr/0012-observabilidade-opt-in-via-otlp.md)).

---

## 1. Os quatro sinais, e a diferença entre eles

| Sinal | O que é | Onde vai | Sempre ativo? |
| --- | --- | --- | :---: |
| **Log** | Evento textual pontual | `stdout` | Sim |
| **Trace** | Árvore de spans de uma requisição | OTLP | **Só com endpoint configurado** |
| **Métrica** | Valor numérico agregado | OTLP | Idem |
| **Sonda** | Estado binário sob demanda | HTTP | Sim |

Distinções que importam para não confundir os sinais:

| Termo | Neste sistema |
| --- | --- |
| **Log** | Um evento por requisição concluída, mais erros. Cardinalidade alta, retenção externa |
| **Métrica** | **Um único** histograma de duração, com três rótulos |
| **Trace** | Span `request` com os `#[instrument]` dos handlers aninhados |
| **Evento de negócio** | **Não existe** — não há "compra realizada" como sinal estruturado |
| **Auditoria** | **Não existe** — nenhum registro de quem alterou o quê |
| **Telemetria de uso** | **Não existe** — nada é coletado sobre comportamento do usuário |
| **Rastreamento distribuído** | **Parcial** — `request_id` é propagado; o contexto W3C `traceparent` **não** é lido |

## 2. Logs

### Formato

Controlado por `LOG_FORMAT`:

| Valor | Saída |
| --- | --- |
| `json` (ignora maiúsculas) | Uma linha JSON por evento — para CloudWatch, Loki, etc. |
| Qualquer outro / ausente | Formato legível de terminal |

O `docker-compose.yaml` define `LOG_FORMAT: json` no serviço `app`.

### Nível

`RUST_LOG`, na sintaxe do `EnvFilter`. Padrão: `info`.

| Exemplo | Efeito |
| --- | --- |
| `info` | Padrão |
| `wallet=debug,info` | Debug só do próprio crate |
| `wallet=trace,sqlx=debug` | Inclui as queries executadas |
| `warn` | Só avisos e erros |

> **`sqlx=debug` registra as queries com parâmetros.** Em produção isso coloca valores
> monetários e identificadores no log. Use com critério.

### Eventos emitidos

| Evento | Nível | Quando | Campos |
| --- | --- | --- | --- |
| `starting service` | `info` | Boot | `bind_addr` |
| `request completed` | `info` | **Toda requisição** | `request_id`, `method`, `path`, `status`, `latency_ms` |
| `internal error serving request` | `error` | Todo 5xx | Erro completo com causa raiz |
| `scheduled quotes sync` | `info` | Rodada bem-sucedida | `assets_updated` |
| `scheduled quotes sync failed` | `warn` | Rodada falha | `error` |
| `scheduled quotes sync disabled` | `info` | Boot com `QUOTES_SYNC_MINUTES=0` | — |
| `shutdown signal received` | `info` | SIGTERM ou Ctrl+C | — |
| Falhas de flush do OTel | `error` | Shutdown | `error` |

O job de mercado emite eventos equivalentes aos de cotações.

### Correlação por `request_id`

Todo log de uma requisição carrega o mesmo `request_id`, e ele **volta na resposta**
no header `x-request-id`.

| Origem | Comportamento |
| --- | --- |
| Header `x-request-id` presente e válido | **Propagado** |
| Ausente ou inválido | Gerado localmente (8 bytes em hexa) |

"Válido" significa: não vazio, ≤ 64 caracteres, só alfanuméricos ASCII e `-` — sem
isso, um header malicioso injetaria lixo nos logs.

**Fluxo de investigação:** o usuário reporta um erro citando o `x-request-id` da
resposta; buscar esse id no log leva à linha exata, com a causa raiz do 5xx.

```bash
docker compose logs app | grep '<request_id>'
```

### Dados que **não** devem aparecer em log

| Dado | Proteção |
| --- | --- |
| Senha | `#[instrument(skip_all)]` — argumentos não entram no span |
| Valores de formulário | Idem |
| Tokens, hashes | Nunca passados a macro de log |
| `JWT_SECRET`, `ADMIN_SECRET_KEY` | Nunca logados |
| `DATABASE_URL` | **Vai inteira** no erro de conexão — lacuna conhecida |

> **Não há verificação automática** de que dado sensível não vaza para o log. A
> proteção é `skip_all` por convenção. Registrado em
> [../security/threat-model.md](../security/threat-model.md).

**Erros 5xx são logados inteiros, com causa raiz** — e é justamente aí que uma
mensagem do SQL pode conter valores de parâmetros. **O log é um ativo sensível.**

## 3. Traces

Ligados **apenas** com `OTEL_EXPORTER_OTLP_ENDPOINT` definida. Sem ela: nenhuma
exportação, nenhuma tentativa de conexão, nenhum overhead.

### Estrutura

```text
span "request"  (request_id, method, path)
 ├─ span do handler        (#[instrument(skip_all)])
 │   └─ spans aninhados
 └─ spans dos middlewares internos
```

O span `request` é aberto pela camada **mais externa**, de propósito: assim até os
logs dos middlewares internos saem correlacionados, e a medição cobre a requisição
inteira, cabeçalhos de segurança inclusos.

### Configuração

| Variável | Padrão | Efeito |
| --- | --- | --- |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | *(desligado)* | Destino OTLP/HTTP |
| `OTEL_SERVICE_NAME` | `wallet` | Nome no backend |

Falha ao montar o exportador **não derruba o boot** — é reportada em `stderr` (não em
`tracing`, porque roda antes de o subscriber global existir) e o serviço segue.

## 4. Métricas

**Uma única métrica é exportada:**

| Sinal | Origem | Finalidade | Unidade | Alerta | Limitação |
| --- | --- | --- | --- | --- | --- |
| `http.server.request.duration` | Middleware `request_tracing` | Latência das requisições | segundos (histograma) | **Nenhum definido** | É a **única** métrica do sistema |

Rótulos:

| Rótulo | Valor | Cuidado |
| --- | --- | --- |
| `http.request.method` | `GET`, `POST`, `PATCH` | — |
| `http.route` | **Padrão da rota** (`/assets`, `/api/v1/assets`) | 404 vira `<unmatched>` |
| `http.response.status_code` | Código numérico | — |

> O rótulo usa o **padrão de rota**, não a URL crua. Usar a URL recebida criaria uma
> série nova para cada caminho aleatório — um scanner varrendo URLs geraria
> cardinalidade ilimitada, e backends cobram por série.

### Métricas que **não** existem

| Métrica ausente | O que não é observável |
| --- | --- |
| Contador de erro por tipo | Taxa de erro por variante de `AppError` |
| Falhas de sincronização de cotação | **Só aparece em log** — não há sinal agregado |
| Latência de chamada externa | Quanto a Coinbase/CoinGecko demora |
| Tamanho do pool de conexões | Saturação do banco |
| Operações por tipo | Volume de depósitos, compras, vendas |
| Sessões ativas | — |
| Idade do snapshot de mercado | Quão defasado está o dado exibido |

Para um sistema financeiro, **a ausência de métrica de negócio é a lacuna mais
relevante**: uma falha prolongada de sincronização de cotações — que congela os preços
que lastreiam operações — só é detectável lendo log.

## 5. Sondas

| Sonda | Toca o banco? | `200` quando | Ação do orquestrador na falha |
| --- | :---: | --- | --- |
| `GET /healthz` | **Não** | Processo responde | **Reiniciar** |
| `GET /readyz` | Sim (`SELECT 1`) | Banco responde | **Tirar do balanceador**, sem reiniciar |
| `GET /health` | Sim | Alias de `/readyz` | Idem |

A separação é operacional: **reiniciar o app não conserta um Postgres fora do ar**.
Uma liveness que dependesse do banco entraria em laço de reinício durante uma
indisponibilidade, piorando o incidente.

Nenhuma exige autenticação nem revela estado além do status HTTP.

## 6. Verificar a exportação localmente

Sem montar um backend de verdade:

```bash
docker compose --profile observability up -d otel-collector
```

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 cargo run
```

```powershell
$env:OTEL_EXPORTER_OTLP_ENDPOINT = 'http://localhost:4318'; cargo run
```

```bash
docker compose logs -f otel-collector
```

O coletor **só imprime** o que recebe — não repassa a lugar nenhum. É uma sonda de
verificação, não um backend.

## 7. Retenção e rotação

| Sinal | Retenção | Responsável |
| --- | --- | --- |
| Logs | **Definida por quem opera** | Docker, orquestrador ou agregador |
| Traces | Definida pelo backend OTLP | Backend |
| Métricas | Idem | Backend |

**A aplicação não gerencia retenção nem rotação de log.** Ela escreve em `stdout`; o
que acontece depois é responsabilidade da infraestrutura. Em Docker, o driver padrão
(`json-file`) cresce sem limite por padrão — configurar `max-size` e `max-file` é
recomendável.

## 8. Como investigar uma falha

| Sintoma | Onde olhar | O que procurar |
| --- | --- | --- |
| Usuário reporta erro | Log, pelo `x-request-id` | `internal error serving request` com a causa raiz |
| Serviço não sobe | Log do boot | Mensagem nomeando a variável, ou erro de migração |
| Lentidão | Métrica de duração, por rota | Qual rota degradou |
| Preços congelados | Log | `scheduled quotes sync failed` |
| Tela de mercado vazia | Log | Falha da rodada de mercado |
| 503 em `/readyz` | Log e o banco | Conexão com o Postgres |

Procedimentos completos em [runbooks.md](runbooks.md).

## 9. Limitações

| # | Limitação | Impacto |
| --- | --- | --- |
| L1 | **Uma métrica só** | Sem taxa de erro, sem métrica de negócio |
| L2 | **Nenhum alerta definido** | O sistema emite sinais; nada diz o que é anormal |
| L3 | `traceparent` de entrada não é lido | Trace que comece num proxy não se conecta |
| L4 | `Drop` é a única garantia de flush | `SIGKILL`/OOM perde o lote em buffer |
| L5 | Sem amostragem configurável | Em volume alto, todos os spans são exportados |
| L6 | Sem verificação de dado sensível em log | Depende de `skip_all` |
| L7 | Falha de cotação só em log | Sem sinal agregado para alertar |
| L8 | Sem métrica de idade do snapshot | Não se sabe quão defasado está o mercado exibido |

## 10. Recomendações

Em ordem de valor por esforço:

1. **Contador de falhas de sincronização de cotação** — hoje o congelamento de preços,
   que afeta operações reais, só é detectável em log.
2. **Contador de erro por variante de `AppError`** — permitiria alertar sobre aumento
   de 5xx.
3. **Definir alertas mínimos**: `/readyz` falhando, taxa de 5xx acima de um piso,
   sincronização de cotação falhando N vezes seguidas.
4. Métrica de idade do snapshot de mercado.
5. Ler `traceparent` de entrada, se houver proxy que o gere.
6. Configurar rotação de log no Docker (`max-size`, `max-file`).

## 11. Evidências

```text
- src/app.rs      · init_tracing, init_otel, OtelGuard, RequestMetrics,
                    request_tracing, new_request_id, REQUEST_ID_HEADER,
                    liveness, readiness
- src/quotes.rs   · spawn_scheduled_sync (eventos de log)
- src/market.rs   · spawn_scheduled_refresh
- src/error.rs    · IntoResponse (log de 5xx com causa raiz)
- docker/otel-collector/config.yaml
- docker-compose.yaml       (perfil observability, LOG_FORMAT: json)
- .env.example              (OTEL_*, LOG_FORMAT, RUST_LOG)
```
