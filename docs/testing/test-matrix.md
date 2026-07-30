# Matriz de testes por risco

## Objetivo

Ler a suíte **pelo risco**, não pelo arquivo: para cada risco técnico identificado,
qual teste o reduz, em que nível, e quais riscos permanecem descobertos.

## Escopo

Coberto: o mapeamento risco → teste, e a lista explícita de riscos sem cobertura. Não
coberto: o detalhe de cada teste (ver [test-catalogue.md](test-catalogue.md)) e a
política de execução (ver [test-plan.md](test-plan.md)).

---

## Legenda

| Campo | Valores |
| --- | --- |
| **Tipo** | U = unidade · I = integração com banco · C = contrato HTTP · P = contrato de payload · R = renderização · S = snapshot |
| **Automação** | CI = roda no pipeline · Manual |

---

## 1. Riscos financeiros

O grupo de maior consequência: uma falha aqui produz valor errado no patrimônio de um
usuário.

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| F-01 | I | `repository` | Depósito válido | Saldo creditado sem registro no extrato | Saldo e transação numa transação só | CI | `deposit_credits_balance_and_logs_transaction` |
| F-02 | I | `repository` | Depósito zero ou negativo | Saque disfarçado de depósito | Recusa | CI | `deposit_rejects_non_positive_amounts` |
| F-03 | I | `repository` | Compra com saldo suficiente | Débito ou posição inconsistentes | Debita e abre posição | CI | `buy_debits_balance_and_opens_holding` |
| F-04 | I | `repository` | Compra sem saldo | **Escrita parcial aplicada** | Recusa e reverte tudo | CI | `buy_rejects_when_balance_is_insufficient` |
| F-05 | I | `repository` | Segunda compra do mesmo ativo | Custo médio errado ⇒ lucro/prejuízo falso | `(2×10+2×20)/4 = 15` | CI | `buying_more_averages_the_cost_basis` |
| F-06 | I | `repository` | Venda total | Posição zerada permanece na carteira | Devolve caixa e apaga a linha | CI | `selling_everything_closes_the_position` |
| F-07 | I | `repository` | Venda parcial | **Recalcular custo médio inventaria lucro** | Custo médio inalterado | CI | `partial_sell_keeps_remaining_units` |
| F-08 | I | `repository` | Venda acima da posição | Venda a descoberto | Recusa | CI | `sell_rejects_more_than_owned` |
| F-09 | I | `repository` | Ativo sem cotação | Negociar a zero = transferência de valor | Recusa | CI | `unquoted_assets_cannot_be_bought_or_sold_for_free` |
| F-10 | I | `repository` | Total arredonda a zero | Moedor de unidades grátis | Nada se move | CI | `trades_that_round_to_zero_do_not_move_cash_or_holdings` |
| F-11 | I | `repository` | Compra de ativo inexistente | Posição órfã | Recusa | CI | `buy_rejects_unknown_asset` |
| F-12 | I | `repository` | Snapshot de patrimônio | Série do gráfico errada | Caixa + posições ao preço do momento | CI | `portfolio_snapshots_capture_cash_plus_holdings` |

**Nenhum destes usa dublê.** Todos rodam contra Postgres efêmero real, porque as
garantias que importam neste nível — transação, `FOR UPDATE`, `CHECK` — são do banco.

## 2. Riscos de precisão monetária

O grupo nascido de um incidente de produção real.

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| P-01 | I | `repository` | **Valores de escala 28 no banco** | Repetição do incidente de 2026-07-22 | Toda leitura decodifica | CI | `legacy_high_scale_money_still_renders_the_wallet` |
| P-02 | I | `repository` | Escrita com escala excessiva | Escala escapando para o banco | Recusa; zeros à direita não contam | CI | `deposits_and_trades_reject_excessive_scale` |
| P-03 | I | `repository` | Preço vindo do admin | Borda administrativa sem trava | Escala travada | CI | `admin_prices_are_capped_at_the_canonical_scale` |
| P-04 | U | `quotes` | Inversão `1/3` (dízima) | 28 casas escapando da inversão | Escala ≤ 8 | CI | `brl_price_caps_the_scale_of_non_terminating_inversions` |
| P-05 | U | `quotes` | Inversão de taxa | **Inverter errado dobra ou divide o patrimônio** | `1/0,2 = 5` | CI | `brl_price_inverts_the_rate_and_rejects_the_uninvertible` |
| P-06 | P | `quotes` | **Taxas reais da Coinbase** | Taxa de 41 dígitos derruba **todos** os pares | 636 taxas decodificam | CI | `the_real_payload_decodes_all_rates_including_the_ones_that_exceed_the_mantissa` |
| P-07 | P | `quotes` | Inversão de taxas reais | Escala que o banco não devolve | Nenhuma acima de `MONEY_SCALE` | CI | `inverting_real_rates_never_leaks_a_scale_the_database_cannot_take_back` |
| P-08 | P | `quotes` | Direção da inversão | Refactor esquecer a inversão | Dólar plausível, bitcoin ordens acima | CI | `the_inversion_direction_is_right_and_not_silently_flipped` |
| P-09 | P | `market` | Decimais do payload real | Escala de 28 casas escapando da fronteira | Preço ≤ 8, variação ≤ 2 | CI | `every_decimal_from_the_payload_arrives_with_its_scale_capped` |
| P-10 | C | `api` | Dinheiro no JSON | `f64` no meio do caminho | **String**, não número | CI | `the_catalogue_round_trips_through_real_http_requests` |

> **P-01 é o teste mais valioso da suíte.** Ele planta deliberadamente o estado que
> causou o incidente e confirma que a correção o sustenta.

## 3. Riscos de autenticação e sessão

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| A-01 | C | rotas privadas | Visitante anônimo | **Dado privado exposto** | `303` para `/login` | CI | `private_screens_send_anonymous_visitors_to_the_login` |
| A-02 | C | htmx | Sessão expirada em fragmento | Login renderizado dentro da carteira | `HX-Redirect` | CI | `an_expired_session_redirects_the_whole_browser_not_just_the_fragment` |
| A-03 | I | `repository` | Rotação de refresh | **Token roubado válido para sempre** | Devolve identidade e queima o antigo | CI | `session_rotation_returns_the_user_and_burns_the_old_token` |
| A-04 | I | `repository` | Rotação após logout | Logout que não revoga | Recusa | CI | `revoked_session_cannot_rotate` |
| A-05 | I | `repository` | Sessão expirada | Expiração ignorada | Recusa | CI | `expired_session_cannot_rotate` |
| A-06 | I | `repository` | Token fabricado | Token forjado vira sessão | Recusa | CI | `unknown_token_cannot_rotate` |
| A-07 | U | `auth::user` | Limites de cadastro | **Campo sem teto = DoS no hash argon2** | Username 3–32, senha 8–128 | CI | `registration_requires_a_reasonable_username_and_password` |
| A-08 | C | fluxo | Cadastro completo | Ciclo cookie → JWT → tela quebrado | Registro abre a carteira | CI | `registering_starts_a_session_that_opens_the_wallet` |
| A-09 | U | `flash` | Erro de credencial | **Mensagem revela quais contas existem** | Mesma mensagem nos dois casos | CI | `business_errors_become_messages_and_internal_errors_do_not` |

## 4. Riscos de autorização

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| Z-01 | C | `api` | Escrita sem credencial | **Estranho altera o preço que lastreia tudo** | 4xx sem credencial, 200 com | CI | `writing_to_the_catalogue_requires_the_admin_credential` |
| Z-02 | C | frontend | Credencial de admin no painel | Divergência entre as duas superfícies | Mesma credencial autoriza | CI | `the_admin_credential_authorises_the_catalogue_route` |
| Z-03 | I | `repository` | Papel padrão | **Privilégio como default** | Padrão `user`; banco recusa outros | CI | `users_default_to_the_user_role_and_can_be_promoted` |

> **Lacuna reconhecida e coberta por desenho:** os testes de unidade de `routes::api`
> constroem o `Admin` à mão, então **a autorização não é exercida ali**.
> `tests/http_api.rs` existe exatamente para fechar esse vão.

## 5. Riscos de CSRF e força bruta

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| C-01 | C | formulários | Token ausente, forjado, vazio | **Operação forjada credita mesmo assim** | 422/303 **e saldo continua zerado** | CI | `forms_without_a_matching_csrf_token_are_refused` |
| C-02 | U | `csrf` | Segunda chamada | Duas abas invalidando uma à outra | Mesmo token | CI | `ensure_generates_and_then_reuses_the_token` |
| C-03 | U | `csrf` | Cookie ausente | **Ausência tratada como permissão** | Recusa | CI | `verify_accepts_matching_token_and_rejects_the_rest` |
| C-04 | U | `csrf` | Aleatoriedade | Token previsível | Não se repetem | CI | `tokens_are_unpredictable` |
| C-05 | U | `throttle` | Abaixo do limite | Defesa atrapalhando usuário legítimo | Login normal funciona | CI | `allows_attempts_below_the_threshold` |
| C-06 | U | `throttle` | Acima do limite | **Caixa do nome burla o lockout** | `  ALICE  ` também bloqueado | CI | `locks_after_too_many_failures` |
| C-07 | U | `throttle` | Login correto | Bloqueio acumulando ao longo do dia | Contador zerado | CI | `success_clears_the_counter` |
| C-08 | U | `throttle` | Backoff | **Sem teto, erro de digitação = horas** | 30s → 60s → … → teto de 15 min | CI | `lock_duration_backs_off_exponentially_with_a_cap` |

> **C-01 verifica o saldo, não só o status.** Conferir apenas o status deixaria passar
> um refactor que redireciona corretamente e credita de qualquer forma.

## 6. Riscos de cabeçalhos, cache e redirecionamento

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| H-01 | C | pilha | Sucesso **e** erro | Cabeçalho de segurança contornado | CSP, nosniff, DENY, no-referrer, no-store | CI | `every_api_response_carries_the_security_headers` |
| H-02 | R | templates | Toda página | **Bloco inline quebra a CSP em produção** | Nenhum `<script>`/`<style>` sem `src` | CI | `pages_carry_no_inline_style_or_script` |
| H-03 | R | assets | Rebuild | **HTML novo com CSS velho** | `ETag`, `no-cache`, 304 | CI | `static_assets_revalidate_by_content_and_answer_304_when_unchanged` |
| H-04 | C | assets | Pela pilha real | Idem, agora montado | 304 com corpo vazio | CI | `static_assets_revalidate_instead_of_being_cached_blind` |
| H-05 | R | `/lang` | `?next=` hostil | **Open redirect com clique que parece do produto** | Só caminho local absoluto | CI | `language_switch_only_follows_local_absolute_paths` |
| H-06 | C | `/lang` | Idem, pela pilha | Idem | Idem | CI | `the_language_switch_is_not_an_open_redirect` |
| H-07 | C | CSV | Download autenticado | **CSV renderizado como texto na tela** | `Content-Disposition: attachment` | CI | `the_statement_exports_as_an_authenticated_csv_download` |

## 7. Riscos de integração externa

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| E-01 | P | `quotes` | Par removido da resposta | **Saída de moeda congela o catálogo** | Pulado; os outros seguem | CI | `a_pair_missing_from_the_response_is_skipped_without_taking_the_others_down` |
| E-02 | P | `quotes` | Corpo inválido, HTML de erro, truncado | **Pânico derruba o serviço** | Erro tipado | CI | `a_malformed_payload_becomes_a_typed_error_not_a_panic` |
| E-03 | P | `market` | Payload real | Ordem à mercê de parâmetro da URL | Ordenado pelo **nosso** critério | CI | `the_real_payload_decodes_into_the_coins_the_screen_consumes` |
| E-04 | P | `market` | Série temporal | "Trocar de moeda é grátis" vira mentira em silêncio | Série vem no mesmo pedido | CI | `the_payload_carries_the_time_series_that_makes_the_chart_free` |
| E-05 | P | `market` | Campo ausente | Moeda útil descartada | Zero neutro; moeda permanece | CI | `missing_fields_degrade_to_neutral_instead_of_dropping_the_coin` |
| E-06 | P | `market` | Preço fora da faixa do dia | Marcador fora do medidor | Dentro de `[10, 590]` | CI | `the_trading_range_marker_stays_inside_the_viewbox_for_real_data` |
| E-07 | U | `market` | Preço `NaN`/`inf`/ausente | Moeda sem preço na tela | Descartada | CI | `moeda_sem_preco_ou_com_preco_invalido_e_descartada` |
| E-08 | C | `/market` | Snapshot vazio | **Tela quebra antes da primeira rodada** | `role="status"` | CI | `the_market_screen_degrades_gracefully_before_the_first_refresh` |
| E-09 | C | `/market` | 7 combinações de query | Parâmetro à mão vira 500 | Sempre 200 | CI | `the_market_screen_accepts_any_state_in_the_query_string` |

## 8. Riscos de contrato e compatibilidade

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| K-01 | S | `api` | Criar, listar, corrigir | **Campo renomeado quebra o consumidor** | Snapshot idêntico | CI | `test_create_asset`, `test_list_assets`, `test_update_asset` |
| K-02 | C | `api` | `/api` vs `/api/v1` | **Alias divergindo com o tempo** | Byte a byte igual | CI | `the_unversioned_alias_serves_the_same_thing_as_v1` |
| K-03 | C | `api` | Spec servida | **Gerador de cliente consome spec malformada** | JSON válido com `openapi` e `paths` | CI | `the_openapi_spec_is_served_and_describes_the_real_routes` |
| K-04 | U | `api` | Cobertura da spec | Rota fora da spec | Três verbos documentados | CI | `openapi_spec_covers_the_asset_routes` |
| K-05 | C | `api` | Entrada inválida | **500 culpa o cliente pelo erro dele** | 4xx, sem "internal server error" | CI | `invalid_payloads_are_rejected_at_the_edge_as_client_errors` |
| K-06 | C | `api` | JSON quebrado, tipo trocado | Handler recebendo lixo | 4xx antes do handler | CI | `malformed_json_bodies_never_reach_the_handler` |
| K-07 | C | `api` | `PATCH` de id inexistente | **200 faz o operador achar que corrigiu** | 404 | CI | `patching_an_unknown_asset_is_a_404` |

## 9. Riscos de interface e acessibilidade

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| U-01 | R | mercado | Variação exibida | **Verde↔vermelho a ΔE ~4,6 sob deuteranopia** | Seta ▲/▼ **e** sinal, sempre | CI | `market_dashboard_marks_direction_with_arrow_and_sign_not_only_colour` |
| U-02 | R | carteira | Fragmento vs página | Base do "operar sem recarregar" quebrada | Fragmento é o alvo exato do swap | CI | `the_wallet_fragment_is_partial_html_embedded_by_the_full_page` |
| U-03 | R | htmx | Restauração de histórico | Voltar/avançar com DOM incompleto | `HX-History-Restore-Request` → página inteira | CI | `htmx_marks_partial_requests_but_history_restore_wants_the_full_page` |
| U-04 | C | htmx | Fragmento vs navegação | Idem, pela pilha | Sem `<!DOCTYPE>` no fragmento | CI | `htmx_requests_get_a_fragment_and_plain_navigation_gets_the_full_page` |
| U-05 | R | scripts | Ordem de carregamento | **Ordem invertida = máscara morta** | `money-input.js` após htmx, ambos `defer` | CI | `money_input_script_loads_after_htmx_with_defer` |
| U-06 | R | depósito | Campo de valor | Máscara virando requisito | `<input type="number">` real | CI | `deposit_amount_field_is_a_plain_number_input_hooked_for_the_mask` |
| U-07 | R | CSV | Formatação | **Excel pt-BR não separa colunas** | `;`, vírgula decimal, aspas dobradas | CI | `csv_export_formats_the_statement_in_ptbr_conventions` |
| U-08 | R | mercado | Valores pequenos e grandes | `R$ 0,00` mentindo como medição | `R$ 0,00004125`; zero → `—` | CI | `compact_scale_keeps_big_aggregates_readable` |
| U-09 | R | mercado | Texto livre em link | **Query se parte em dois** | `&`, `=`, espaço percent-encoded | CI | `market_urls_percent_encode_free_text` |
| U-10 | U | `i18n` | Tags de idioma | Navegador manda de tudo | `PT-br`, `pt-BR`, `pt` iguais | CI | `tags_resolve_ignoring_case_and_region` |
| U-11 | U | `i18n` | Idioma não suportado | Erro em vez de fallback | Primeiro **suportado** da lista | CI | `accept_language_takes_the_first_supported_entry` |
| U-12 | U | `flash` | Acento no cookie | Texto corrompido; flash reaparecendo | Sobrevive; uso único | CI | `flash_roundtrips_through_the_cookie_including_accents` |

## 10. Riscos operacionais

| ID | Tipo | Componente | Cenário | Risco validado | Resultado esperado | Autom. | Evidência |
| --- | :---: | --- | --- | --- | --- | :---: | --- |
| O-01 | C | sondas | Liveness vs readiness | **Reinício em laço durante queda do banco** | `/healthz` não toca o banco | CI | `liveness_and_readiness_are_separate_probes` |
| O-02 | C | fluxo | Depositar → comprar → vender | Produto quebrado com as camadas montadas | Cada passo com CSRF renovado | CI | `the_money_flow_works_end_to_end_through_the_browser_path` |
| O-03 | C | operações | Compra sem saldo | **"Erro interno" em vez de "sem saldo"** | 303 com banner, nunca 5xx | CI | `a_business_error_comes_back_as_a_banner_not_a_500` |
| O-04 | I | `repository` | Bootstrap do catálogo | Segunda linha duplicando alias do admin | Respeita o que já existe | CI | `market_bootstrap_inserts_once_and_respects_existing_aliases` |
| O-05 | I | `repository` | Paginação do extrato | **Ordem instável some com linhas** | 3→2, depois a mais antiga | CI | `transactions_paginate_newest_first_without_gaps` |

## 11. Riscos **sem** cobertura

A parte mais importante desta matriz. Cada linha é um risco **conhecido e aceito**,
não uma omissão.

| ID | Risco | Impacto | Por que não é coberto | Prioridade |
| --- | --- | --- | --- | --- |
| N-01 | **Reversão de migração falha** | Rollback impossível no pior momento | Nenhum teste executa os 11 `.down.sql` | **Alta** |
| N-02 | **Query nova de agregado sem `ROUND`** | Repetição do incidente de 500 | O teste de regressão pega o **estado antigo**, não uma query nova | **Alta** |
| N-03 | **`config.rs` sem teste** | Serviço subindo sem segredo validado | Nunca foi escrito | **Alta** |
| N-04 | Erro de runtime no htmx | Operação inerte para quem tem JS | Nada executa JavaScript | Média |
| N-05 | `holdings` divergindo de `transactions` | Posição não bate com o histórico | Nenhuma reconciliação existe | Média |
| N-06 | Corrida no `RwLock`/`Mutex` dos jobs | Snapshot ou sincronização inconsistentes | Sem teste de concorrência | Média |
| N-07 | Imagem Docker não sobe | Deploy quebrado | O CI prova que **compila**, não que **serve** | Média |
| N-08 | Layout quebrado | Interface inutilizável em alguma largura | Só a presença de classes é conferida | Baixa |
| N-09 | Degradação sob carga | Latência inaceitável | Sem medição | Baixa |
| N-10 | Perda prolongada de conectividade | Comportamento desconhecido | Sem simulação | Baixa |

Todos constam de [../decisions/technical-debt.md](../decisions/technical-debt.md) ou
[../decisions/known-limitations.md](../decisions/known-limitations.md).

## 12. Cobertura por objetivo do plano

| Objetivo | Testes | Coberto |
| --- | ---: | --- |
| O1 — nenhuma operação inconsistente | 12 (F-01…F-12) | **Sim** |
| O2 — nenhuma perda de exatidão | 10 (P-01…P-10) | **Sim** |
| O3 — nenhuma rota privada aberta | 2 (A-01, A-02) | **Sim** |
| O4 — nenhuma escrita sem autorização | 3 (Z-01…Z-03) | **Sim** |
| O5 — contrato JSON congelado | 4 (K-01…K-04) | **Sim** |
| O6 — mudança na fonte detectada | 9 (E-01…E-09) | **Sim** |
| O7 — CSP continua fechável | 2 (H-01, H-02) | **Sim** |
| O8 — direção nunca só por cor | 1 (U-01) | **Sim** |

**Os oito objetivos têm cobertura.** As lacunas da §11 são riscos que **não** estão
entre os objetivos declarados — o que é uma escolha consciente de escopo, e por isso
mesmo precisa estar escrita.
