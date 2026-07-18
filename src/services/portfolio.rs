use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use rust_decimal::Decimal;

use crate::app::AppState;
use crate::error::AppError;
use crate::models::{Asset, Holding, PortfolioSnapshot, Transaction, WalletSummary};
use crate::repository::Repository;

/// Transações por página do extrato.
pub const TRANSACTIONS_PAGE_SIZE: i64 = 25;

/// Pontos de série carregados para o gráfico de evolução.
const CHART_POINTS: i64 = 60;

/// Tudo o que a tela da carteira precisa, montado numa chamada só.
pub struct WalletView {
    pub summary: WalletSummary,
    pub holdings: Vec<Holding>,
    pub available_assets: Vec<Asset>,
    pub transactions: Vec<Transaction>,
    pub page: u32,
    pub has_prev: bool,
    pub has_next: bool,
    pub chart: EquityChart,
}

/// Série do patrimônio pronta para desenhar: os pontos já vêm projetados no
/// viewBox do SVG (100×32), então o template só interpola a string — zero
/// JavaScript, amigável à CSP.
pub struct EquityChart {
    pub points: String,
    pub min_value: Decimal,
    pub max_value: Decimal,
    pub latest_value: Decimal,
    has_data: bool,
}

impl EquityChart {
    /// O gráfico só aparece com dois ou mais pontos (um ponto não é uma linha).
    pub fn has_data(&self) -> bool {
        self.has_data
    }

    fn empty() -> Self {
        Self {
            points: String::new(),
            min_value: Decimal::ZERO,
            max_value: Decimal::ZERO,
            latest_value: Decimal::ZERO,
            has_data: false,
        }
    }
}

/// Projeta a série no viewBox 100×32 (margem de 2): x distribui os pontos
/// uniformemente, y escala entre o mínimo e o máximo da janela. Série constante
/// vira uma linha reta no meio — sem divisão por zero.
fn equity_chart(snapshots: &[PortfolioSnapshot]) -> EquityChart {
    if snapshots.len() < 2 {
        return EquityChart::empty();
    }

    let values: Vec<Decimal> = snapshots.iter().map(|s| s.total_value).collect();
    let min = *values.iter().min().expect("non-empty");
    let max = *values.iter().max().expect("non-empty");
    let range = max - min;

    let last_index = (values.len() - 1) as f64;
    let points = values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let x = 2.0 + (i as f64 / last_index) * 96.0;
            let y = if range.is_zero() {
                16.0
            } else {
                // Converte para f64 SÓ para desenhar (coordenadas de tela);
                // os valores exibidos continuam Decimal exato.
                let ratio = ((*value - min) / range)
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.5);
                30.0 - ratio * 28.0
            };
            format!("{x:.2},{y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ");

    EquityChart {
        points,
        min_value: min,
        max_value: max,
        latest_value: *values.last().expect("non-empty"),
        has_data: true,
    }
}

/// Serviço da carteira do usuário: orquestra as operações (depósito, compra,
/// venda) e a montagem da visão da carteira. Os handlers ficam responsáveis só
/// pelo HTTP (formulários, CSRF, redirects); o repository, só pelo SQL.
///
/// Também é um extrator do Axum, como o `Repository` — declarar um parâmetro
/// `PortfolioService` num handler é tudo que é preciso para usá-lo.
pub struct PortfolioService {
    repository: Repository,
}

impl PortfolioService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    /// Monta a visão completa da carteira. As cinco consultas são independentes
    /// entre si, então rodam concorrentes (`try_join!`) — o tempo total é o da
    /// consulta mais lenta, não a soma de todas.
    pub async fn wallet_view(&self, user_id: i64, page: u32) -> Result<WalletView, AppError> {
        let page = page.max(1);
        let offset = i64::from(page - 1) * TRANSACTIONS_PAGE_SIZE;

        let (summary, holdings, available_assets, transactions, total_transactions, snapshots) = tokio::try_join!(
            self.repository.wallet_summary(user_id),
            self.repository.list_holdings(user_id),
            self.repository.list_assets(),
            self.repository
                .list_transactions(user_id, TRANSACTIONS_PAGE_SIZE, offset),
            self.repository.count_transactions(user_id),
            self.repository
                .list_portfolio_snapshots(user_id, CHART_POINTS)
        )?;

        let has_next = has_next_page(offset, transactions.len(), total_transactions);

        Ok(WalletView {
            summary,
            holdings,
            available_assets,
            transactions,
            page,
            has_prev: page > 1,
            has_next,
            chart: equity_chart(&snapshots),
        })
    }

    pub async fn deposit(&self, user_id: i64, amount: Decimal) -> Result<(), AppError> {
        self.repository.deposit(user_id, amount).await
    }

    pub async fn buy(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError> {
        self.repository.buy_asset(user_id, asset_id, quantity).await
    }

    pub async fn sell(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError> {
        self.repository
            .sell_asset(user_id, asset_id, quantity)
            .await
    }
}

/// Existe próxima página se o que já foi mostrado (offset + itens desta página)
/// ainda não cobre o total.
fn has_next_page(offset: i64, page_len: usize, total: i64) -> bool {
    (offset + page_len as i64) < total
}

impl FromRequestParts<AppState> for PortfolioService {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self::new(Repository::from_state(state)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use time::macros::datetime;

    fn snapshot(value: Decimal) -> PortfolioSnapshot {
        PortfolioSnapshot {
            total_value: value,
            captured_at: datetime!(2026-07-18 12:00 UTC),
        }
    }

    #[test]
    fn equity_chart_scales_the_series_into_the_viewbox() {
        let chart = equity_chart(&[
            snapshot(dec!(100)),
            snapshot(dec!(200)),
            snapshot(dec!(150)),
        ]);

        assert!(chart.has_data());
        assert_eq!(chart.min_value, dec!(100));
        assert_eq!(chart.max_value, dec!(200));
        assert_eq!(chart.latest_value, dec!(150));

        let points: Vec<&str> = chart.points.split(' ').collect();
        assert_eq!(points.len(), 3);
        // Primeiro ponto na margem esquerda, no fundo (valor mínimo);
        // segundo no meio, no topo (valor máximo).
        assert_eq!(points[0], "2.00,30.00");
        assert_eq!(points[1], "50.00,2.00");
        assert!(points[2].starts_with("98.00,"));
    }

    #[test]
    fn equity_chart_handles_flat_and_short_series() {
        // Série constante: linha reta no meio, sem divisão por zero.
        let flat = equity_chart(&[snapshot(dec!(50)), snapshot(dec!(50))]);
        assert!(flat.has_data());
        assert_eq!(flat.points, "2.00,16.00 98.00,16.00");

        // Menos de dois pontos não formam linha.
        assert!(!equity_chart(&[snapshot(dec!(50))]).has_data());
        assert!(!equity_chart(&[]).has_data());
    }

    #[test]
    fn next_page_math_covers_the_edges() {
        // 25 mostrados de 27: ainda há próxima.
        assert!(has_next_page(0, 25, 27));
        // Página 2 mostrou os 2 restantes: acabou.
        assert!(!has_next_page(25, 2, 27));
        // Exatamente uma página cheia: não inventa página vazia.
        assert!(!has_next_page(0, 25, 25));
        // Extrato vazio.
        assert!(!has_next_page(0, 0, 0));
    }
}
