use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Serialize)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub unit_value: Decimal,
}

pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub balance: Decimal,
}

pub struct WalletSummary {
    pub balance: Decimal,
    pub holdings_value: Decimal,
    pub total_value: Decimal,
    pub total_invested: Decimal,
    pub total_delta: Decimal,
}

pub struct Holding {
    pub id: i64,
    pub name: String,
    pub unit_value: Decimal,
    pub quantity_owned: Decimal,
    pub avg_cost: Decimal,
    pub current_value: Decimal,
    pub invested_value: Decimal,
    pub value_delta: Decimal,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PurchaseRecord {
    #[serde(with = "time::serde::rfc3339")]
    pub bought_at: OffsetDateTime,
    pub unit_value: Decimal,
    pub quantity: Decimal,
    pub value_delta: Decimal,
}

pub struct Transaction {
    pub id: i64,
    pub kind: String,
    pub asset_name: Option<String>,
    pub quantity: Option<Decimal>,
    pub unit_value: Option<Decimal>,
    pub cash_delta: Decimal,
    pub created_at: OffsetDateTime,
}
