# ADR-0012: Observabilidade opt-in via OTLP, sem custo quando desligada

## Status

Aceita. Amplia a observabilidade anterior, que era apenas `tracing` para o terminal.

## Contexto

O estado anterior era `tracing` escrevendo no terminal, com nível controlável por
`RUST_LOG`. Isso permite ler o que aconteceu, mas não responde três perguntas que
importam em operação:

1. **Quanto tempo as requisições levam**, agregado — não linha por linha de log.
2. **Como correlacionar** um erro reportado pelo usuário com a linha de log
   correspondente no servidor.
3. **Como enxergar isso fora do terminal**, num backend que agregue e alerte.

A dificuldade é que o projeto **não tem backend de observabilidade em operação**. Uma
solução que exigisse um Jaeger ou um Prometheus de pé para o serviço funcionar
transformaria infraestrutura auxiliar em dependência do caminho crítico — num sistema
que serve requisições financeiras.

## Restrições

- O serviço tem de rodar **exatamente igual** sem nenhum backend configurado:
  nenhuma tentativa de conexão, nenhum overhead, nenhum erro no log.
- Falha de infraestrutura de observabilidade **não pode** derrubar o serviço.
- Instrumentação não deve exigir ramificação (`if exportando { ... }`) no caminho
  quente.
- Métricas não podem ter cardinalidade ilimitada — um backend cobra por série.
- A escolha do backend deve ficar com quem opera, não com o código.

## Opções consideradas

**Avaliadas de fato:**

1. **Só `tracing` no terminal** — estado anterior.
2. **OpenTelemetry via OTLP, opt-in por variável de ambiente** — decisão adotada.

**Comparação *post hoc***:

3. **Prometheus com endpoint `/metrics`** — modelo pull, exigiria expor uma rota e
   não daria traces; seriam dois mecanismos para dois sinais.
4. Cliente de APM proprietário — acoplaria o código a um fornecedor.
5. Sem observabilidade — diagnóstico dependeria de leitura de log linha a linha.

## Decisão

**OpenTelemetry via OTLP/HTTP**, ligado **apenas** se
`OTEL_EXPORTER_OTLP_ENDPOINT` estiver definida. São exportados:

- **Traces** — span `request` por requisição (com `request_id`, método, caminho), e
  os spans dos handlers `#[instrument]` aninhados dentro.
- **Uma métrica** — histograma `http.server.request.duration`, em segundos, rotulado
  por método, **padrão de rota** e status.

Sem a variável: nenhuma exportação, nenhuma tentativa de conexão. Os instrumentos
locais continuam funcionando como operações sem efeito.

Logs seguem por `tracing`, com formato selecionável por `LOG_FORMAT` (`json` para
agregadores, texto legível para terminal).

## Fundamentação

**Motivo confirmado**, e a decisão de projeto mais interessante deste componente
está no comentário de `RequestMetrics`:

> "Construídos a partir do `Meter` GLOBAL (`opentelemetry::global`): se nenhum
> `MeterProvider` foi instalado (ver `init_otel`), os handles funcionam do mesmo
> jeito, só que descartam tudo que é registrado — **sem `Option`, sem ramificação no
> caminho quente**, o serviço nunca precisa saber se a exportação está ligada."

Isso é o que torna o "opt-in sem custo" real em vez de retórico: não há `if` a pagar
por requisição, e o código de instrumentação é idêntico nos dois modos.

**Motivo confirmado** para não derrubar o boot em falha de exportador:

> "Falha ao montar um exportador (endpoint malformado, por exemplo) NÃO derruba o
> boot, ao contrário de um segredo obrigatório ausente em `Config`: observabilidade é
> infraestrutura auxiliar, não algo pelo qual vale a pena recusar servir requisições
> financeiras."

E um detalhe de implementação que revela cuidado: essa falha é reportada com
`eprintln!`, não `tracing::error!`, "porque isto roda antes de o subscriber global
existir" — a mesma razão pela qual `LOG_FORMAT` é lido cru, sem passar pela `Config`.
O logging precisa existir **antes** de a configuração ser validada, para que os
próprios erros de configuração já saiam no formato certo.

**Motivo confirmado** para rotular a métrica pelo padrão de rota, não pela URL:

> "Um 404 não tem `MatchedPath`. Usar a URL recebida aqui criaria uma série
> diferente para cada caminho aleatório; o log ainda preserva `path`, mas a métrica
> mantém cardinalidade limitada."

Caminho não roteado vira `<unmatched>`. Sem isso, um scanner varrendo URLs
aleatórias criaria uma série de métrica por tentativa.

**Por que `request_tracing` é a camada mais externa.** Assim até os logs dos
middlewares internos saem correlacionados ao mesmo `request_id`, e a métrica de
duração cobre a requisição inteira — cabeçalhos de segurança inclusos.

**Validação do `request_id` externo.** O id vem do header `x-request-id` quando um
proxy já o gerou, mas só é aceito se não vazio, com ≤ 64 caracteres e apenas
alfanuméricos ASCII ou `-`. Sem isso, um header malicioso injetaria conteúdo
arbitrário nos logs. Ele é **sempre devolvido na resposta**, para que o cliente possa
citá-lo num reporte de erro e o log correspondente seja encontrado na hora.

**Por que o `OtelGuard` existe.** Nenhum dos providers expõe flush pela ponta do
`tracing`, então o `Drop` é a **única** garantia de que o lote em buffer sai antes do
processo morrer — inclusive numa saída antecipada por erro, quando o `?` de
`App::start` desmonta a stack e derruba o guard no caminho.

**Duas sondas com propósitos distintos**, e a distinção é operacional:

| Sonda | Toca o banco? | Ação do orquestrador na falha |
| --- | --- | --- |
| `/healthz` (liveness) | **Não** | **Reiniciar** o container |
| `/readyz`, `/health` (readiness) | Sim (`SELECT 1`) | **Tirar do balanceador**, sem reiniciar |

O motivo de a liveness não tocar o banco está registrado: "reiniciar o app não
conserta um Postgres fora do ar."

**Coletor local de verificação.** `docker compose --profile observability up
otel-collector` sobe um coletor que **só imprime** o que recebe, sem repassar a
lugar nenhum. Serve para confirmar que a exportação está saindo do serviço, sem
montar um backend de verdade.

## Consequências positivas

- Serviço roda idêntico com ou sem backend, sem overhead quando desligado.
- Sem ramificação no caminho quente.
- Falha de observabilidade nunca impede servir requisições.
- Escolha do backend fica com quem opera (qualquer receptor OTLP).
- Cardinalidade de métrica limitada por construção.
- `request_id` correlaciona reporte do usuário com log do servidor.
- Logs em JSON para agregadores, com uma variável.
- Sondas separadas permitem ao orquestrador reagir corretamente a cada tipo de
  falha.
- Verificação local possível sem infraestrutura.

## Consequências negativas

- **Uma métrica só.** Apenas o histograma de duração é exportado. Não há contador de
  erro por tipo, gauge do pool de conexões, nem métrica de negócio (operações por
  tipo, falhas de sincronização de cotação). Para um sistema financeiro, a ausência
  de métrica de negócio é a lacuna mais relevante.
- **Sem tracing distribuído de entrada.** O `request_id` é propagado, mas o contexto
  W3C `traceparent` **não** é lido de requisições upstream — então um trace que
  comece num proxy não se conecta ao span daqui.
- **Quatro crates com versões acopladas** (`opentelemetry`, `opentelemetry-otlp`,
  `opentelemetry_sdk`, `tracing-opentelemetry`): atualizar um exige atualizar os
  quatro.
- **`Drop` é a única garantia de flush.** Um `abort` do processo (SIGKILL, OOM) perde
  o lote em buffer — justamente nos casos em que os dados seriam mais úteis.
- **Nenhum alerta definido.** O sistema exporta sinais; não há regra que diga o que é
  anormal.
- **`LOG_FORMAT` e as variáveis OTEL não passam pela `Config`**, então não são
  validadas com o mesmo rigor: um valor inesperado em `LOG_FORMAT` cai
  silenciosamente no formato de texto.
- Não há amostragem configurável: em volume alto, todos os spans são exportados.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Endpoint OTLP inacessível em produção | Baixo — serviço segue servindo | Falha não derruba o boot; erro em `eprintln!` |
| Dado sensível vazando em span ou log | **Médio** | `#[instrument(skip_all)]` nos handlers, que evita registrar argumentos (senha, valor de formulário). **Não há verificação automática disso** |
| Perda de telemetria em `abort` | Baixo | `OtelGuard` cobre saída normal e por erro; não cobre SIGKILL |
| Ausência de métrica de negócio | Médio — falha de sincronização de cotação só aparece em log | **Nenhuma.** Registrado como débito técnico |
| Cardinalidade explodindo | Baixo | Rótulo por `MatchedPath`, com `<unmatched>` para 404 |

## Evidências

```text
- Cargo.toml                        (opentelemetry 0.32, otlp, sdk 0.32.1,
                                     tracing-opentelemetry 0.33)
- src/app.rs                        · init_tracing, init_otel, OtelGuard,
                                      RequestMetrics, request_tracing,
                                      new_request_id, REQUEST_ID_HEADER,
                                      liveness, readiness
- src/config.rs                     (LOG_FORMAT e OTEL_* deliberadamente fora)
- docker/otel-collector/config.yaml (coletor de verificação, só imprime)
- docker-compose.yaml               (perfil observability)
- .env.example                      (OTEL_EXPORTER_OTLP_ENDPOINT comentado)
- testes: liveness_and_readiness_are_separate_probes
```

## Critérios de revisão

Reavaliar quando:

1. **Houver um backend de observabilidade em operação de fato** — aí vale acrescentar
   métricas de negócio (operações por tipo, falhas de sincronização, latência de
   chamada externa) e definir alertas.
2. O sistema receber tráfego de um proxy que já gere `traceparent` — aí ler o
   contexto W3C de entrada passa a ser necessário para o trace não quebrar na borda.
3. O volume tornar a exportação de todos os spans custosa — aí configurar amostragem.
4. A ausência de contador de erro dificultar um diagnóstico real.
