# ADR-0011: Versionamento de API por caminho, com alias de compatibilidade

## Status

Aceita.

## Contexto

A API administrativa nasceu sob `/api/assets`, sem versão. Isso significa que
qualquer mudança incompatível — renomear um campo, mudar um tipo, alterar o
significado de uma resposta — quebraria consumidores existentes sem caminho de
migração.

O consumidor conhecido é uma integração máquina-a-máquina que altera
`assets.unit_value` — **o preço que lastreia toda compra e venda**
([ADR-0008](0008-autorizacao-por-papel-e-credencial-de-servico.md)). Uma quebra
silenciosa aqui não é um inconveniente de integração: é um caminho para preço errado
em produção.

Ao mesmo tempo, havia um consumidor **já em uso** no caminho sem versão, e romper com
ele para introduzir versionamento seria exatamente o problema que o versionamento
existe para evitar.

## Restrições

- O caminho `/api` já estava em uso e não podia ser rompido.
- O projeto tem um único serviço; não há gateway nem proxy que possa reescrever
  caminhos.
- A especificação OpenAPI é gerada do código (`utoipa`) e precisa refletir o caminho
  canônico.
- Autor único: a solução não pode exigir manutenção paralela de duas implementações.

## Opções consideradas

**Avaliadas de fato:**

1. **Manter `/api` sem versão** — estado inicial, insustentável para mudança
   incompatível.
2. **Versão no caminho** (`/api/v1/...`), com `/api` mantido como alias — decisão
   adotada.
3. **Substituir `/api` por `/api/v1`** sem alias — romperia o consumidor existente.

**Comparação *post hoc***:

4. Versão em header (`Accept: application/vnd.wallet.v1+json`) — mais correto do
   ponto de vista de REST, invisível na URL, mais difícil de testar com `curl` e de
   documentar.
5. Versão em query string (`?version=1`) — mistura versão de contrato com parâmetro
   de requisição.

## Decisão

Caminho canônico **`/api/v1`**, com **`/api` mantido como alias de
compatibilidade**. Ambos são montados a partir do **mesmo** `Router`:

```rust
.nest("/api/v1", crate::routes::api::router())
.nest("/api",    crate::routes::api::router())
```

A especificação OpenAPI documenta apenas o caminho canônico (`/api/v1/assets`), e é
servida em `/api/v1/openapi.json`.

## Fundamentação

**Motivo confirmado** — o comentário em `App::router` enuncia a intenção:

> "Caminho canônico e versionado da API: mudanças incompatíveis futuras entram como
> `/api/v2` sem quebrar consumidores do v1."
>
> "Alias de compatibilidade para consumidores existentes de `/api`."

**A decisão de implementação que sustenta a garantia** é montar o mesmo `Router`
duas vezes, em vez de duplicar handlers. Isso torna a divergência entre os dois
caminhos **estruturalmente impossível** enquanto o v1 for a versão corrente — não há
código separado que possa receber uma correção e o outro não.

E há um teste que trava exatamente isso:
`the_unversioned_alias_serves_the_same_thing_as_v1` compara as duas respostas **byte
a byte**. A justificativa registrada no catálogo de testes é precisa:
"versionamento só vale se o alias não divergir com o tempo."

**Por que caminho e não header.** Versão em header é tecnicamente mais elegante, mas
o caminho tem três vantagens práticas neste contexto: é visível em log de acesso,
testável com `curl` sem cabeçalho extra, e documentável numa spec OpenAPI sem
recorrer a `content negotiation`. Para um consumidor máquina-a-máquina único, a
elegância do header não paga o custo de opacidade.

**Por que a spec só documenta `/api/v1`.** O alias existe para compatibilidade, não
como interface recomendada. Documentá-lo daria a entender que os dois são
equivalentes permanentemente — e não são: quando o v2 existir, o alias precisará
apontar para uma versão específica ou ser descontinuado.

**Estabilidade do contrato é travada por snapshot.** As respostas JSON são congeladas
com `insta` (3 snapshots versionados em `src/routes/snapshots/`). Uma mudança de
formato exige `cargo insta review` explícito — nunca passa despercebida. E o teste
`the_catalogue_round_trips_through_real_http_requests` confirma que **dinheiro sai
como string JSON**, o que é parte do contrato, não detalhe de implementação
([ADR-0004](0004-decimal-e-numeric-para-dinheiro.md)).

## Consequências positivas

- Um `/api/v2` pode ser introduzido sem tocar no v1.
- Consumidores de `/api` continuam funcionando.
- Divergência entre alias e canônico é impossível por construção, e verificada por
  teste.
- A spec OpenAPI é gerada do código: a documentação não pode descolar da
  implementação.
- Mudança de formato de resposta exige aprovação explícita de snapshot.

## Consequências negativas

- **O alias `/api` é ambíguo por natureza.** Hoje serve o v1; quando existir um v2,
  será necessário decidir se ele continua no v1, migra para o v2 (quebrando
  consumidores) ou é descontinuado. **Essa decisão não está tomada nem documentada
  como política.**
- **Não há política de descontinuação.** Nada define por quanto tempo uma versão é
  mantida, como o consumidor é avisado, nem se haverá header de aviso
  (`Deprecation`, `Sunset`).
- **Duas rotas para o mesmo recurso** dobram a superfície a considerar em qualquer
  análise de segurança — ainda que o código seja o mesmo.
- **A versão não cobre o frontend.** As rotas SSR não são versionadas, o que é
  adequado (não há consumidor programático), mas significa que "a API é versionada" é
  verdadeiro só para `/api/*`.
- A spec não descreve o alias, então uma ferramenta que gere cliente a partir dela
  nunca usará `/api` — o que é o comportamento desejado, mas cria uma discrepância
  entre o que existe e o que está documentado.

## Riscos

| Risco | Impacto | Mitigação atual |
| --- | --- | --- |
| Alias divergir do canônico | Médio | **Eliminado por construção** (mesmo Router) + teste byte a byte |
| Consumidor preso ao alias quando o v2 chegar | Médio | **Nenhuma.** Requer definir política de descontinuação |
| Mudança de contrato passar despercebida | Médio | 3 snapshots `insta`; `cargo insta review` obrigatório |
| Spec desatualizada em relação ao código | Baixo | Gerada do código; `openapi_spec_covers_the_asset_routes` |

## Evidências

```text
- src/app.rs             · App::router (nest de /api/v1 e /api, com o porquê)
- src/routes/api.rs      · router, ApiDoc (info.version = "1.0.0")
- src/routes/snapshots/  (3 snapshots insta versionados)
- testes: the_unversioned_alias_serves_the_same_thing_as_v1,
          the_openapi_spec_is_served_and_describes_the_real_routes,
          openapi_spec_covers_the_asset_routes,
          the_catalogue_round_trips_through_real_http_requests
```

## Critérios de revisão

Reavaliar **antes** de introduzir a primeira mudança incompatível — que é o momento
em que as lacunas desta decisão passam a doer. Especificamente, é preciso definir:

1. **O destino do alias `/api`**: permanece no v1, migra, ou é descontinuado?
2. **Política de descontinuação**: por quanto tempo cada versão é mantida, e como o
   consumidor é avisado (headers `Deprecation`/`Sunset`, changelog, aviso na spec).
3. Se a API se tornar pública ou multi-tenant, revisar também o versionamento por
   header, que permite evolução mais granular.

Reavaliar também se a API deixar de ter consumidor programático conhecido — nesse
caso o custo do alias deixa de se justificar.
