//! Contrato com a CoinGecko: o payload REAL atravessa o mesmo parse que o job
//! de produção atravessa.
//!
//! O que esta bateria protege que os testes de unidade em `src/market.rs` não
//! podem proteger: lá os `MarketRow` são construídos campo a campo, então
//! provam que a nossa lógica funciona **sobre a nossa ideia do formato**. Aqui a
//! entrada é a resposta que a fonte devolveu de verdade (ver
//! `tests/payloads/README.md`), com os 30 campos, os `null` e os tipos que ela
//! realmente manda. Se a CoinGecko renomear `price_change_percentage_24h`,
//! passar a mandar preço como string ou parar de mandar `sparkline_in_7d`, o
//! erro aparece aqui — em CI, com nome de campo no texto — e não como uma tela
//! vazia em produção.

use rust_decimal::Decimal;
use wallet::market::{Coin, Range};
use wallet::models::MONEY_SCALE;

/// A captura versionada. `include_str!` embute em tempo de compilação: se
/// alguém apagar o payload, o teste não compila em vez de sumir da suíte.
const PAYLOAD: &str = include_str!("payloads/coingecko_markets.json");

fn coins() -> Vec<Coin> {
    wallet::market::parse_markets(PAYLOAD).expect("o payload real precisa decodificar")
}

/// **O teste de contrato propriamente dito.** Tudo o que a tela mostra sai
/// destes campos; se um deles mudar de nome ou de tipo, o `parse` falha e este
/// teste é o primeiro a dizer.
#[test]
fn the_real_payload_decodes_into_the_coins_the_screen_consumes() {
    let coins = coins();

    assert_eq!(coins.len(), 4, "quatro moedas na captura");

    // Ordenado por ranking pela NOSSA ordenação, não pela ordem do JSON: a
    // fonte já manda ordenado por capitalização, mas depender disso deixaria a
    // lista à mercê de um parâmetro na URL.
    let ranks: Vec<i64> = coins.iter().map(|coin| coin.rank).collect();
    assert_eq!(ranks, [1, 2, 3, 4]);

    let btc = &coins[0];
    assert_eq!(
        btc.id, "bitcoin",
        "o id da fonte é a chave de seleção da tela"
    );
    assert_eq!(btc.symbol, "BTC", "o ticker sai normalizado em maiúsculas");
    assert_eq!(btc.name, "Bitcoin");

    // Preço é o único campo sem o qual a moeda é descartada; ele tem de estar
    // presente e positivo em todas as quatro.
    for coin in &coins {
        assert!(coin.price > Decimal::ZERO, "{}: sem preço", coin.id);
    }
}

/// A fronteira de precisão. A fonte manda número JSON (que o serde decodifica
/// como `f64`, trazendo o erro de representação do binário); a escala tem de
/// ser travada NA ENTRADA, antes de o valor circular.
///
/// Existe porque um `Decimal` de 28 casas escapando dessa fronteira já derrubou
/// o resumo da carteira uma vez: os agregados no SQL ficaram indecodificáveis na
/// volta.
#[test]
fn every_decimal_from_the_payload_arrives_with_its_scale_capped() {
    for coin in coins() {
        for (field, value) in [
            ("price", coin.price),
            ("high_24h", coin.high_24h),
            ("low_24h", coin.low_24h),
            ("ath", coin.ath),
        ] {
            assert!(
                value.scale() <= MONEY_SCALE,
                "{}.{field}: escala {} passa de {MONEY_SCALE}",
                coin.id,
                value.scale()
            );
        }

        for (field, value) in [
            ("change_1h", coin.change_1h),
            ("change_24h", coin.change_24h),
            ("change_7d", coin.change_7d),
            ("ath_change_pct", coin.ath_change_pct),
            ("market_cap", coin.market_cap),
            ("volume_24h", coin.volume_24h),
            ("circulating_supply", coin.circulating_supply),
        ] {
            assert!(
                value.scale() <= 2,
                "{}.{field}: variação e agregado ficam em 2 casas, veio com {}",
                coin.id,
                value.scale()
            );
        }
    }
}

/// A série temporal vem no MESMO pedido da lista (`sparkline=true`), e é essa
/// carona que sustenta a promessa de que trocar de moeda ou de janela não custa
/// chamada externa. Se a fonte parar de mandar a série, o gráfico some — e a
/// promessa vira mentira em silêncio. Este teste é o alarme.
#[test]
fn the_payload_carries_the_time_series_that_makes_the_chart_free() {
    let coins = coins();
    let updated_at = Some(time::macros::datetime!(2026-07-29 12:00 UTC));

    for coin in &coins {
        let week = coin
            .chart(Range::Week, updated_at)
            .unwrap_or_else(|| panic!("{}: sem série semanal no payload", coin.id));
        let day = coin
            .chart(Range::Day, updated_at)
            .unwrap_or_else(|| panic!("{}: sem janela de 24 h", coin.id));

        // As duas janelas saem da mesma série: a de 24 h é o rabo dela, então a
        // faixa do dia nunca pode ser MAIS larga que a da semana.
        assert!(
            day.max - day.min <= week.max - week.min,
            "{}: a janela de 24 h não pode exceder a de 7 d",
            coin.id
        );

        // O caminho projetado é o que vai no atributo `d` de um `<path>`: tem de
        // começar num `M` e não conter nada além de geometria.
        assert!(week.line.starts_with('M'), "{}: caminho inválido", coin.id);
        assert!(
            !week.line.contains("NaN") && !week.line.contains("inf"),
            "{}: coordenada não finita vazaria para o SVG",
            coin.id
        );

        // Cinco marcas de tempo, equidistantes como as amostras que datam.
        assert_eq!(week.ticks.len(), 5, "{}: eixo do tempo", coin.id);
        assert_eq!(day.ticks.len(), 5);
        // Janela de um dia é rotulada por hora; a de sete, por data.
        assert!(day.ticks[0].contains(':'), "24 h rotula por hora");
        assert!(week.ticks[0].contains('/'), "7 d rotula por data");
    }
}

/// O medidor da faixa de negociação é geometria de SVG porque a CSP fecha
/// `style-src` em `'self'` — não existe `style="width:63%"` nesta interface. A
/// coordenada tem de cair dentro do `viewBox` para TODA moeda do payload real,
/// inclusive quando a fonte publica preço fora da faixa (ela apura preço,
/// mínima e máxima em momentos diferentes; acontece).
#[test]
fn the_trading_range_marker_stays_inside_the_viewbox_for_real_data() {
    for coin in coins() {
        let Some(marker) = coin.trading_range_x() else {
            // Sem faixa publicada não há medidor — omitir é correto.
            continue;
        };

        let x: f64 = marker.parse().expect("coordenada numérica");
        assert!(
            (10.0..=590.0).contains(&x),
            "{}: marcador em {x} sai do medidor (viewBox 0 0 600 12)",
            coin.id
        );
    }
}

/// Campo que a fonte não publica vira ZERO, e a moeda **permanece** na tela pelo
/// preço. É a decisão que mantém uma linha útil em vez de descartá-la — e o
/// template lê zero como "não informado", desenhando um traço.
///
/// O payload real é a única fonte honesta para isto: moeda recém-listada sem
/// série de 24 h existe lá, não num fixture que a gente lembrou de escrever.
#[test]
fn missing_fields_degrade_to_neutral_instead_of_dropping_the_coin() {
    let coins = coins();
    let payload_ids: Vec<&str> = coins.iter().map(|coin| coin.id.as_str()).collect();

    // Nenhuma moeda com preço foi descartada pelo caminho.
    assert!(payload_ids.contains(&"bitcoin"));
    assert!(payload_ids.contains(&"ethereum"));

    // Zero é representável e não é NaN: o template pode formatá-lo sem checar.
    for coin in &coins {
        assert!(coin.market_cap >= Decimal::ZERO);
        assert!(coin.circulating_supply >= Decimal::ZERO);
    }
}

/// A busca da lista lateral casa ticker E nome, com o termo já normalizado.
/// Contra dado real, porque é aí que "Tether" vs "USDT" e maiúsculas importam.
#[test]
fn search_matches_ticker_and_name_on_real_coins() {
    let coins = coins();
    let find = |needle: &str| coins.iter().filter(|coin| coin.matches(needle)).count();

    assert_eq!(find("btc"), 1, "por ticker");
    assert_eq!(find("bitcoin"), 1, "por nome");
    assert_eq!(find("zzzznaoexiste"), 0);
    // Termo parcial casa mais de uma: é o comportamento esperado da busca.
    assert!(find("e") >= 2);
}

/// Payload que não é o esperado tem de virar erro tipado, não pânico. Um 502 da
/// fonte, uma página de manutenção em HTML ou um JSON truncado no meio: o job
/// registra a falha, marca o snapshot como defasado e a tela continua mostrando
/// o último bom.
#[test]
fn a_malformed_payload_becomes_a_typed_error_not_a_panic() {
    for body in [
        "",
        "não é json",
        "<html>maintenance</html>",
        "{}",                          // objeto onde esperamos lista
        "[{\"id\":\"x\"}]",            // faltam campos obrigatórios
        &PAYLOAD[..PAYLOAD.len() / 2], // truncado no meio
    ] {
        assert!(
            wallet::market::parse_markets(body).is_err(),
            "corpo inválido tinha de falhar: {:?}",
            &body[..body.len().min(40)]
        );
    }

    // Lista vazia é JSON válido: decodifica sem erro, e o snapshot vazio é o que
    // faz a tela mostrar "buscando as cotações" em vez de quebrar.
    assert_eq!(
        wallet::market::parse_markets("[]")
            .expect("lista vazia é válida")
            .len(),
        0
    );
}
