//! Contrato com a API de câmbio da Coinbase: o payload REAL atravessa o mesmo
//! parse que alimenta `assets.unit_value` — o preço que **lastreia compra, venda
//! e saldo**.
//!
//! Esta é a integração mais perigosa do projeto, e o perigo está na
//! decodificação, não na aritmética:
//!
//! 1. A Coinbase entrega cada taxa como **string**, não número. É de propósito
//!    do lado deles e é o que nos permite ir direto a `Decimal` sem passar por
//!    ponto flutuante — mas significa que a precisão é arbitrária: na captura
//!    versionada há taxa com **41 dígitos significativos**, mais que os 28 da
//!    mantissa do `Decimal`.
//! 2. O mapa é decodificado **de uma vez**, com 636 moedas. Uma única taxa que
//!    não caiba faz `serde_json::from_str` falhar e derruba a sincronização de
//!    TODOS os pares — não só a da moeda problemática.
//!
//! Os testes de unidade em `src/quotes.rs` montam o `HashMap` à mão: provam que
//! a inversão está certa, e não podem provar que a resposta de hoje decodifica.
//! É essa lacuna que esta bateria fecha.

use std::collections::HashMap;

use rust_decimal::Decimal;
use wallet::models::MONEY_SCALE;
use wallet::quotes::{MARKET_PAIRS, brl_price, parse_brl_rates};

const PAYLOAD: &str = include_str!("payloads/coinbase_exchange_rates.json");

fn rates() -> HashMap<String, Decimal> {
    parse_brl_rates(PAYLOAD).expect("o payload real precisa decodificar por inteiro")
}

/// **O teste que justifica a captura.** 636 taxas, a maior com 41 dígitos
/// significativos, todas num único `from_str`. Se o `Decimal` recusar uma delas
/// em vez de arredondar, a sincronização inteira morre — e o sintoma em produção
/// seria "os preços pararam de atualizar", sem nada apontando para a causa.
#[test]
fn the_real_payload_decodes_all_rates_including_the_ones_that_exceed_the_mantissa() {
    let rates = rates();

    assert!(
        rates.len() > 500,
        "a captura tem centenas de moedas; veio com {}",
        rates.len()
    );

    // A taxa mais longa da captura: 41 dígitos significativos contra os 28 da
    // mantissa. Ela existe no arquivo — se o parse a recusasse, `rates()` acima
    // já teria estourado.
    let ooki = rates.get("OOKI").expect("OOKI está na captura");
    assert!(*ooki > Decimal::ZERO, "arredondada, não zerada nem perdida");

    // Os pares que o serviço realmente usa têm de estar presentes: é a lista
    // canônica de `MARKET_PAIRS`, não uma cópia dela neste teste.
    for (code, canonical_name, _) in MARKET_PAIRS {
        assert!(
            rates.contains_key(*code),
            "{canonical_name} ({code}) saiu da resposta de câmbio"
        );
    }
}

/// O arredondamento em `brl_price` não é cosmético: a divisão de `Decimal`
/// preenche a mantissa inteira, e um preço com escala 28 gravado no banco torna
/// os produtos e somas do resumo da carteira indecodificáveis na volta. Foi
/// exatamente o incidente do 500 em `/assets`.
///
/// Contra dado real porque é a taxa real que produz a dízima: 1/0,195043… não
/// termina.
#[test]
fn inverting_real_rates_never_leaks_a_scale_the_database_cannot_take_back() {
    let rates = rates();

    for (code, canonical_name, _) in MARKET_PAIRS {
        let price = brl_price(&rates, code)
            .unwrap_or_else(|| panic!("{canonical_name}: taxa real não inverteu"));

        assert!(
            price.scale() <= MONEY_SCALE,
            "{canonical_name}: escala {} passa de {MONEY_SCALE}",
            price.scale()
        );
        assert!(
            price > Decimal::ZERO,
            "{canonical_name}: preço não positivo"
        );
    }
}

/// Sanidade de grandeza sobre dado real, sem prender o teste à cotação do dia
/// (o que exigiria recaptura semanal e viraria alarme falso): o dólar em reais
/// mora numa faixa larga mas finita, e o bitcoin vale muitas ordens de grandeza
/// mais que ele. Se a inversão fosse esquecida em algum refactor, o dólar
/// apareceria como 0,19 — e esta faixa pegaria.
#[test]
fn the_inversion_direction_is_right_and_not_silently_flipped() {
    let rates = rates();

    let usd = brl_price(&rates, "USD").expect("USD");
    let btc = brl_price(&rates, "BTC").expect("BTC");

    assert!(
        usd > Decimal::ONE && usd < Decimal::from(100),
        "1 USD em BRL fora de qualquer faixa plausível: {usd}"
    );
    assert!(
        btc > usd * Decimal::from(1000),
        "bitcoin tem de valer ordens de grandeza mais que o dólar: {btc} vs {usd}"
    );
}

/// Par ausente da resposta é PULADO, e os demais atualizam mesmo assim. É a
/// decisão que evita que a saída de uma moeda da API congele o catálogo inteiro.
#[test]
fn a_pair_missing_from_the_response_is_skipped_without_taking_the_others_down() {
    let mut rates = rates();
    rates.remove("SOL");

    assert_eq!(
        brl_price(&rates, "SOL"),
        None,
        "par removido não inventa preço"
    );
    assert!(brl_price(&rates, "BTC").is_some(), "os outros seguem");

    // Taxa não positiva não é invertível — e zero seria uma divisão por zero.
    rates.insert("BTC".to_string(), Decimal::ZERO);
    assert_eq!(brl_price(&rates, "BTC"), None);
    rates.insert("BTC".to_string(), Decimal::from(-1));
    assert_eq!(brl_price(&rates, "BTC"), None);
}

/// Resposta que não é o esperado vira erro tipado, não pânico: a rodada falha, é
/// logada, e a próxima tenta de novo. Cotação atrasada não derruba o serviço.
#[test]
fn a_malformed_payload_becomes_a_typed_error_not_a_panic() {
    for body in [
        "",
        "não é json",
        "<html>503</html>",
        "{}",                                     // sem o envelope `data`
        "{\"data\":{}}",                          // sem `rates`
        "{\"data\":{\"rates\":{\"USD\":\"x\"}}}", // taxa que não é número
        &PAYLOAD[..PAYLOAD.len() / 2],            // truncado no meio
    ] {
        assert!(
            parse_brl_rates(body).is_err(),
            "corpo inválido tinha de falhar: {:?}",
            &body[..body.len().min(40)]
        );
    }
}
