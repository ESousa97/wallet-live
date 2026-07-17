use rust_decimal::Decimal;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

#[derive(Clone, Serialize, ToSchema)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    /// Preço unitário atual, decimal exato serializado como string.
    pub unit_value: Decimal,
}

/// Papel de admin em `users.role` (o padrão do banco é `'user'`).
pub const ROLE_ADMIN: &str = "admin";

/// Identidade mínima de um usuário (sem hash de senha nem saldo). É o que a
/// rotação de sessão devolve para reconstruir o `User` autenticado.
pub struct UserIdentity {
    pub id: i64,
    pub username: String,
    pub role: String,
}

pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub balance: Decimal,
    pub role: String,
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

pub struct Transaction {
    pub id: i64,
    pub kind: String,
    pub asset_name: Option<String>,
    pub quantity: Option<Decimal>,
    pub unit_value: Option<Decimal>,
    pub cash_delta: Decimal,
    pub created_at: OffsetDateTime,
}
