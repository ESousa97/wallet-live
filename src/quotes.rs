use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;

use crate::error::AppError;
use crate::repository::Repository;

#[derive(Deserialize)]
struct CoinbaseRatesResponse {
    data: CoinbaseRates,
}

#[derive(Deserialize)]
struct CoinbaseRates {
    rates: HashMap<String, Decimal>,
}

pub async fn sync_market_quotes(repository: &Repository) -> Result<usize, AppError> {
    let (usd_brl, btc_brl) = tokio::try_join!(fetch_usd_brl(), fetch_btc_brl())?;

    let mut updates = HashMap::new();
    updates.insert("real", Decimal::ONE);
    updates.insert("brl", Decimal::ONE);
    updates.insert("dolar", usd_brl);
    updates.insert("dólar", usd_brl);
    updates.insert("usd", usd_brl);
    updates.insert("bitcoin", btc_brl);
    updates.insert("btc", btc_brl);

    repository.update_known_asset_prices(&updates).await
}

async fn fetch_usd_brl() -> Result<Decimal, AppError> {
    fetch_coinbase_rate("USD", "BRL").await
}

async fn fetch_btc_brl() -> Result<Decimal, AppError> {
    fetch_coinbase_rate("BTC", "BRL").await
}

async fn fetch_coinbase_rate(base: &str, quote: &str) -> Result<Decimal, AppError> {
    let url = format!("https://api.coinbase.com/v2/exchange-rates?currency={base}");
    let response: CoinbaseRatesResponse =
        reqwest::get(url).await?.error_for_status()?.json().await?;

    response
        .data
        .rates
        .get(quote)
        .copied()
        .ok_or(AppError::QuoteUnavailable)
}
