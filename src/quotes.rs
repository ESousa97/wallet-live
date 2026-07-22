use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{info, warn};

use crate::app::AppState;
use crate::error::AppError;
use crate::repository::Repository;

/// Sobe o job periódico de cotações: uma rodada imediata no boot e depois uma a
/// cada `QUOTES_SYNC_MINUTES` (zero desliga). Falha de rodada é logada e a
/// próxima tenta de novo — cotação atrasada não derruba o serviço.
pub fn spawn_scheduled_sync(state: AppState) {
    let minutes = state.config.quotes_sync_minutes;
    if minutes == 0 {
        info!("scheduled quotes sync disabled");
        return;
    }

    tokio::spawn(async move {
        let repository = Repository::from_state(&state);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(minutes * 60));

        loop {
            // O primeiro tick resolve na hora: o boot já sincroniza.
            interval.tick().await;
            match sync_quotes_round(&repository).await {
                Ok(updated) => info!(assets_updated = updated, "scheduled quotes sync"),
                Err(error) => warn!(?error, "scheduled quotes sync failed"),
            }
        }
    });
}

/// Uma rodada completa de cotações: atualiza os preços E fotografa o patrimônio
/// de todos os usuários (a série do gráfico de evolução). Usada tanto pelo job
/// agendado quanto pelo botão manual — os dois caminhos alimentam o histórico.
pub async fn sync_quotes_round(repository: &Repository) -> Result<usize, AppError> {
    let updated = sync_market_quotes(repository).await?;
    repository.record_portfolio_snapshots().await?;
    Ok(updated)
}

#[derive(Deserialize)]
struct CoinbaseRatesResponse {
    data: CoinbaseRates,
}

#[derive(Deserialize)]
struct CoinbaseRates {
    rates: HashMap<String, Decimal>,
}

/// Pares suportados: código na API de câmbio → nomes de ativo que casam no
/// catálogo (normalizados para minúsculas pelo repository).
const MARKET_PAIRS: &[(&str, &[&str])] = &[
    ("USD", &["dolar", "dólar", "usd"]),
    ("EUR", &["euro", "eur"]),
    ("BTC", &["bitcoin", "btc"]),
    ("ETH", &["ethereum", "eth"]),
    ("SOL", &["solana", "sol"]),
];

/// Atualiza os preços do catálogo com as cotações de mercado. UMA chamada só:
/// a API devolve todas as taxas BRL→moeda de uma vez, e o preço em BRL de cada
/// par é o inverso da taxa (BRL→USD = 0,2 ⇒ 1 USD = 5 BRL). Um par ausente na
/// resposta é pulado — os demais atualizam mesmo assim.
pub async fn sync_market_quotes(repository: &Repository) -> Result<usize, AppError> {
    let rates = fetch_brl_rates().await?;

    let mut updates = HashMap::new();
    updates.insert("real", Decimal::ONE);
    updates.insert("brl", Decimal::ONE);

    for (code, names) in MARKET_PAIRS {
        if let Some(price) = brl_price(&rates, code) {
            for name in *names {
                updates.insert(name, price);
            }
        }
    }

    repository.update_known_asset_prices(&updates).await
}

/// Todas as taxas partindo do BRL (BRL→moeda) numa requisição.
async fn fetch_brl_rates() -> Result<HashMap<String, Decimal>, AppError> {
    let response: CoinbaseRatesResponse =
        reqwest::get("https://api.coinbase.com/v2/exchange-rates?currency=BRL")
            .await?
            .error_for_status()?
            .json()
            .await?;

    Ok(response.data.rates)
}

/// Preço em BRL de uma unidade da moeda `code`: o inverso da taxa BRL→moeda.
/// Taxa ausente ou não positiva (não dá para inverter) vira `None`.
///
/// O arredondamento para `MONEY_SCALE` é OBRIGATÓRIO: a divisão de `Decimal`
/// preenche a mantissa inteira (28 dígitos), e um preço com escala 28 gravado
/// no banco torna os produtos/somas do resumo da carteira indecodificáveis na
/// volta (foi exatamente o incidente do 500 em /assets).
fn brl_price(rates: &HashMap<String, Decimal>, code: &str) -> Option<Decimal> {
    let rate = rates.get(code)?;
    if *rate <= Decimal::ZERO {
        return None;
    }
    Some((Decimal::ONE / rate).round_dp(crate::models::MONEY_SCALE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn brl_price_inverts_the_rate_and_rejects_the_uninvertible() {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), dec!(0.2)); // 1 BRL = 0,2 USD
        rates.insert("ZERO".to_string(), dec!(0));

        // 1 USD = 1 / 0,2 = 5 BRL.
        assert_eq!(brl_price(&rates, "USD"), Some(dec!(5)));
        // Taxa zero não é invertível; moeda desconhecida não existe.
        assert_eq!(brl_price(&rates, "ZERO"), None);
        assert_eq!(brl_price(&rates, "XYZ"), None);
    }

    #[test]
    fn brl_price_caps_the_scale_of_non_terminating_inversions() {
        let mut rates = HashMap::new();
        // 1/3 é dízima: sem arredondar, a divisão de Decimal devolve 28 casas
        // e o preço gravado explodiria a decodificação dos agregados no SQL.
        rates.insert("USD".to_string(), dec!(3));
        rates.insert("BTC".to_string(), dec!(0.0000029937));

        for code in ["USD", "BTC"] {
            let price = brl_price(&rates, code).expect("price");
            assert!(
                price.scale() <= crate::models::MONEY_SCALE,
                "{code}: escala {} > {}",
                price.scale(),
                crate::models::MONEY_SCALE
            );
        }
    }
}
