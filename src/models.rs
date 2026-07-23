use rust_decimal::Decimal;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

/// Escala canônica de dinheiro/preço: TODA gravação monetária arredonda para
/// até 8 casas decimais (sub-centavo suficiente até para cripto). O invariante
/// existe porque `NUMERIC` do Postgres é ilimitado, mas `rust_decimal::Decimal`
/// tem 28 dígitos significativos: valores de escala alta (ex.: preço = 1/taxa
/// com 28 casas) tornam PRODUTOS e SOMAS no SQL indecodificáveis na leitura
/// (`value not representable`) — derrubando a tela da carteira.
pub const MONEY_SCALE: u32 = 8;

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

#[derive(Clone)]
pub struct WalletSummary {
    pub balance: Decimal,
    pub holdings_value: Decimal,
    pub total_value: Decimal,
    pub total_invested: Decimal,
    pub total_delta: Decimal,
}

impl WalletSummary {
    /// Resultado dos ativos em %, sobre o que foi investido. `None` quando não
    /// há nada investido: percentual sobre base zero não existe, e é
    /// exatamente o estado da carteira recém-criada — a interface mostra o
    /// valor absoluto e omite o percentual em vez de exibir "0%" ou "∞".
    pub fn delta_pct(&self) -> Option<Decimal> {
        percent_of(self.total_delta, self.total_invested)
    }
}

#[derive(Clone)]
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

impl Holding {
    /// Resultado da posição em %, sobre o custo médio investido nela.
    pub fn delta_pct(&self) -> Option<Decimal> {
        percent_of(self.value_delta, self.invested_value)
    }
}

/// `parte / base` em pontos percentuais, com duas casas. Base zero devolve
/// `None` em vez de estourar a divisão.
pub fn percent_of(part: Decimal, base: Decimal) -> Option<Decimal> {
    if base.is_zero() {
        return None;
    }
    Some((part / base * Decimal::ONE_HUNDRED).round_dp(2))
}

/// Um ponto da série do patrimônio (usado pelo gráfico de evolução).
#[derive(Clone)]
pub struct PortfolioSnapshot {
    pub total_value: Decimal,
    pub captured_at: OffsetDateTime,
}

#[derive(Clone)]
pub struct Transaction {
    pub id: i64,
    pub kind: String,
    pub asset_name: Option<String>,
    pub quantity: Option<Decimal>,
    pub unit_value: Option<Decimal>,
    pub cash_delta: Decimal,
    pub created_at: OffsetDateTime,
}
