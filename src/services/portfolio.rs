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

/// Dimensões do viewBox do gráfico. A proporção é fixa e o SVG escala
/// uniformemente (sem `preserveAspectRatio="none"`), para que o marcador do
/// último ponto continue redondo em qualquer largura de tela.
const CHART_W: f64 = 600.0;
const CHART_H: f64 = 160.0;
/// Margem interna: o marcador tem raio 5 e ganha um anel de 2px, então precisa
/// de folga para não ser cortado pela borda do viewBox.
const CHART_PAD_X: f64 = 10.0;
const CHART_PAD_Y: f64 = 14.0;

/// Série do patrimônio pronta para desenhar: os caminhos já vêm projetados no
/// viewBox do SVG, então o template só interpola strings — zero JavaScript,
/// amigável à CSP.
pub struct EquityChart {
    /// Caminho da linha (`d` de um `<path>`).
    pub line: String,
    /// Mesmo caminho fechado contra a base, para o preenchimento em wash.
    pub area: String,
    /// Coordenadas do último ponto, onde vai o marcador.
    pub last_x: String,
    pub last_y: String,
    pub min_value: Decimal,
    pub max_value: Decimal,
    pub latest_value: Decimal,
    /// Variação do primeiro ao último ponto da janela, em %.
    pub delta_pct: Option<Decimal>,
    has_data: bool,
}

impl EquityChart {
    /// O gráfico só aparece com dois ou mais pontos (um ponto não é uma linha).
    pub fn has_data(&self) -> bool {
        self.has_data
    }

    /// A série subiu no período? Decide a cor do traço — sempre acompanhada do
    /// percentual com sinal, nunca cor sozinha.
    pub fn is_up(&self) -> bool {
        self.delta_pct.unwrap_or(Decimal::ZERO) > Decimal::ZERO
    }

    pub fn is_down(&self) -> bool {
        self.delta_pct.unwrap_or(Decimal::ZERO) < Decimal::ZERO
    }

    /// Gráfico vazio (também usado nos testes de renderização do front-end).
    pub(crate) fn empty() -> Self {
        Self {
            line: String::new(),
            area: String::new(),
            last_x: "0".to_string(),
            last_y: "0".to_string(),
            min_value: Decimal::ZERO,
            max_value: Decimal::ZERO,
            latest_value: Decimal::ZERO,
            delta_pct: None,
            has_data: false,
        }
    }
}

/// Projeta a série no viewBox: x distribui os pontos uniformemente, y escala
/// entre o mínimo e o máximo da janela. Série constante vira uma linha reta no
/// meio — sem divisão por zero.
fn equity_chart(snapshots: &[PortfolioSnapshot]) -> EquityChart {
    if snapshots.len() < 2 {
        return EquityChart::empty();
    }

    let values: Vec<Decimal> = snapshots.iter().map(|s| s.total_value).collect();
    let min = *values.iter().min().expect("non-empty");
    let max = *values.iter().max().expect("non-empty");
    let range = max - min;

    let last_index = (values.len() - 1) as f64;
    let span_x = CHART_W - 2.0 * CHART_PAD_X;
    let span_y = CHART_H - 2.0 * CHART_PAD_Y;

    let coords: Vec<(f64, f64)> = values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let x = CHART_PAD_X + (i as f64 / last_index) * span_x;
            let y = if range.is_zero() {
                CHART_H / 2.0
            } else {
                // Converte para f64 SÓ para desenhar (coordenadas de tela);
                // os valores exibidos continuam Decimal exato.
                let ratio = ((*value - min) / range)
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.5);
                CHART_PAD_Y + (1.0 - ratio) * span_y
            };
            (x, y)
        })
        .collect();

    let line = coords
        .iter()
        .enumerate()
        .map(|(i, (x, y))| {
            let verb = if i == 0 { 'M' } else { 'L' };
            format!("{verb}{x:.2} {y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ");

    // O preenchimento é a mesma linha fechada até a base do viewBox.
    let (first_x, _) = coords[0];
    let (last_x, last_y) = *coords.last().expect("non-empty");
    let area = format!("{line} L{last_x:.2} {CHART_H:.2} L{first_x:.2} {CHART_H:.2} Z");

    let first_value = *values.first().expect("non-empty");
    let latest_value = *values.last().expect("non-empty");

    EquityChart {
        line,
        area,
        last_x: format!("{last_x:.2}"),
        last_y: format!("{last_y:.2}"),
        min_value: min,
        max_value: max,
        latest_value,
        delta_pct: crate::models::percent_of(latest_value - first_value, first_value),
        has_data: true,
    }
}

/// O subconjunto do `Repository` que o `PortfolioService` precisa. Existe para
/// que o serviço possa ser testado com um dublê em memória, sem banco — o
/// `Repository` real implementa este trait delegando para seus métodos
/// inerentes (a resolução de métodos do Rust prefere o inerente, então não há
/// recursão).
// `wallet` não expõe uma lib crate — o trait não cruza fronteira de crate, então
// a falta de bound `Send` explícito (o que o lint cobra) não arrisca um
// implementador não-`Send` vazar para o runtime multi-thread do axum; o único
// implementador de produção (`Repository`) já é `Send` e isso é checado onde
// os handlers o usam.
#[allow(async_fn_in_trait)]
pub trait PortfolioRepository {
    async fn wallet_summary(&self, user_id: i64) -> Result<WalletSummary, AppError>;
    async fn list_holdings(&self, user_id: i64) -> Result<Vec<Holding>, AppError>;
    async fn list_assets(&self) -> Result<Vec<Asset>, AppError>;
    async fn list_transactions(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>, AppError>;
    async fn count_transactions(&self, user_id: i64) -> Result<i64, AppError>;
    async fn list_portfolio_snapshots(
        &self,
        user_id: i64,
        limit: i64,
    ) -> Result<Vec<PortfolioSnapshot>, AppError>;
    async fn deposit(&self, user_id: i64, amount: Decimal) -> Result<(), AppError>;
    async fn buy_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError>;
    async fn sell_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError>;
}

impl PortfolioRepository for Repository {
    async fn wallet_summary(&self, user_id: i64) -> Result<WalletSummary, AppError> {
        Ok(self.wallet_summary(user_id).await?)
    }

    async fn list_holdings(&self, user_id: i64) -> Result<Vec<Holding>, AppError> {
        Ok(self.list_holdings(user_id).await?)
    }

    async fn list_assets(&self) -> Result<Vec<Asset>, AppError> {
        Ok(self.list_assets().await?)
    }

    async fn list_transactions(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>, AppError> {
        Ok(self.list_transactions(user_id, limit, offset).await?)
    }

    async fn count_transactions(&self, user_id: i64) -> Result<i64, AppError> {
        Ok(self.count_transactions(user_id).await?)
    }

    async fn list_portfolio_snapshots(
        &self,
        user_id: i64,
        limit: i64,
    ) -> Result<Vec<PortfolioSnapshot>, AppError> {
        Ok(self.list_portfolio_snapshots(user_id, limit).await?)
    }

    async fn deposit(&self, user_id: i64, amount: Decimal) -> Result<(), AppError> {
        self.deposit(user_id, amount).await
    }

    async fn buy_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError> {
        self.buy_asset(user_id, asset_id, quantity).await
    }

    async fn sell_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError> {
        self.sell_asset(user_id, asset_id, quantity).await
    }
}

/// Serviço da carteira do usuário: orquestra as operações (depósito, compra,
/// venda) e a montagem da visão da carteira. Os handlers ficam responsáveis só
/// pelo HTTP (formulários, CSRF, redirects); o repository, só pelo SQL.
///
/// Genérico sobre `PortfolioRepository` (padrão para `Repository`, o que
/// mantém `PortfolioService` usável sem `<...>` em toda a base) para que os
/// testes possam injetar um dublê em memória em vez do Postgres.
///
/// Também é um extrator do Axum, como o `Repository` — declarar um parâmetro
/// `PortfolioService` num handler é tudo que é preciso para usá-lo.
pub struct PortfolioService<R: PortfolioRepository = Repository> {
    repository: R,
}

impl<R: PortfolioRepository> PortfolioService<R> {
    pub fn new(repository: R) -> Self {
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
        // Zero representa "ainda sem cotação" no catálogo. Ele continua
        // visível na API administrativa, mas não aparece como opção de compra:
        // uma operação financeira nunca pode ser aberta gratuitamente.
        let available_assets = available_assets
            .into_iter()
            .filter(|asset| asset.unit_value > Decimal::ZERO)
            .collect();

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
    use std::cell::RefCell;

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

        // Primeiro ponto na margem esquerda e no fundo (valor mínimo); segundo
        // no meio e no topo (valor máximo); o último fecha na margem direita.
        assert_eq!(chart.line, "M10.00 146.00 L300.00 14.00 L590.00 80.00");

        // O preenchimento é a mesma linha fechada contra a base do viewBox.
        assert_eq!(
            chart.area,
            "M10.00 146.00 L300.00 14.00 L590.00 80.00 L590.00 160.00 L10.00 160.00 Z"
        );

        // O marcador fica sobre o último ponto da linha.
        assert_eq!(
            (chart.last_x.as_str(), chart.last_y.as_str()),
            ("590.00", "80.00")
        );

        // Variação do período: de 100 para 150 é +50%, e a série sobe.
        assert_eq!(chart.delta_pct, Some(dec!(50)));
        assert!(chart.is_up());
    }

    #[test]
    fn equity_chart_handles_flat_and_short_series() {
        // Série constante: linha reta no meio, sem divisão por zero.
        let flat = equity_chart(&[snapshot(dec!(50)), snapshot(dec!(50))]);
        assert!(flat.has_data());
        assert_eq!(flat.line, "M10.00 80.00 L590.00 80.00");
        // Sem variação: linha neutra, sem fingir alta.
        assert_eq!(flat.delta_pct, Some(dec!(0)));
        assert!(!flat.is_up());
        assert!(!flat.is_down());

        // Série que cai pinta de vermelho e reporta o percentual negativo.
        let down = equity_chart(&[snapshot(dec!(200)), snapshot(dec!(150))]);
        assert_eq!(down.delta_pct, Some(dec!(-25)));
        assert!(!down.is_up());
        assert!(down.is_down());

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

    /// Dublê de `Repository` para testar a orquestração do `PortfolioService`
    /// sem banco. Devolve exatamente os dados configurados, sem lógica própria —
    /// a matemática financeira (custo médio, saldo, guardas) já tem cobertura
    /// contra o Postgres real em `repository.rs`; aqui o alvo é só a montagem
    /// da `WalletView` e a propagação de erro das operações.
    struct FakeRepository {
        summary: WalletSummary,
        holdings: Vec<Holding>,
        assets: Vec<Asset>,
        transactions: Vec<Transaction>,
        transaction_count: i64,
        snapshots: Vec<PortfolioSnapshot>,
        deposit_result: RefCell<Result<(), AppError>>,
        buy_result: RefCell<Result<(), AppError>>,
        sell_result: RefCell<Result<(), AppError>>,
    }

    impl Default for FakeRepository {
        fn default() -> Self {
            Self {
                summary: WalletSummary {
                    balance: Decimal::ZERO,
                    holdings_value: Decimal::ZERO,
                    total_value: Decimal::ZERO,
                    total_invested: Decimal::ZERO,
                    total_delta: Decimal::ZERO,
                },
                holdings: Vec::new(),
                assets: Vec::new(),
                transactions: Vec::new(),
                transaction_count: 0,
                snapshots: Vec::new(),
                deposit_result: RefCell::new(Ok(())),
                buy_result: RefCell::new(Ok(())),
                sell_result: RefCell::new(Ok(())),
            }
        }
    }

    impl PortfolioRepository for FakeRepository {
        async fn wallet_summary(&self, _user_id: i64) -> Result<WalletSummary, AppError> {
            Ok(self.summary.clone())
        }

        async fn list_holdings(&self, _user_id: i64) -> Result<Vec<Holding>, AppError> {
            Ok(self.holdings.clone())
        }

        async fn list_assets(&self) -> Result<Vec<Asset>, AppError> {
            Ok(self.assets.clone())
        }

        async fn list_transactions(
            &self,
            _user_id: i64,
            _limit: i64,
            _offset: i64,
        ) -> Result<Vec<Transaction>, AppError> {
            Ok(self.transactions.clone())
        }

        async fn count_transactions(&self, _user_id: i64) -> Result<i64, AppError> {
            Ok(self.transaction_count)
        }

        async fn list_portfolio_snapshots(
            &self,
            _user_id: i64,
            _limit: i64,
        ) -> Result<Vec<PortfolioSnapshot>, AppError> {
            Ok(self.snapshots.clone())
        }

        async fn deposit(&self, _user_id: i64, _amount: Decimal) -> Result<(), AppError> {
            self.deposit_result.replace(Ok(()))
        }

        async fn buy_asset(
            &self,
            _user_id: i64,
            _asset_id: i64,
            _quantity: Decimal,
        ) -> Result<(), AppError> {
            self.buy_result.replace(Ok(()))
        }

        async fn sell_asset(
            &self,
            _user_id: i64,
            _asset_id: i64,
            _quantity: Decimal,
        ) -> Result<(), AppError> {
            self.sell_result.replace(Ok(()))
        }
    }

    fn holding(name: &str) -> Holding {
        Holding {
            id: 1,
            name: name.to_string(),
            unit_value: dec!(10),
            quantity_owned: dec!(2),
            avg_cost: dec!(8),
            current_value: dec!(20),
            invested_value: dec!(16),
            value_delta: dec!(4),
        }
    }

    fn transaction() -> Transaction {
        Transaction {
            id: 1,
            kind: "deposit".to_string(),
            asset_name: None,
            quantity: None,
            unit_value: None,
            cash_delta: dec!(1),
            created_at: datetime!(2026-07-18 12:00 UTC),
        }
    }

    #[tokio::test]
    async fn wallet_view_assembles_repository_data_and_paginates() {
        let fake = FakeRepository {
            summary: WalletSummary {
                balance: dec!(70),
                holdings_value: dec!(30),
                total_value: dec!(100),
                total_invested: dec!(20),
                total_delta: dec!(10),
            },
            holdings: vec![holding("bitcoin")],
            assets: vec![
                Asset {
                    id: 1,
                    name: "bitcoin".to_string(),
                    unit_value: dec!(10),
                },
                Asset {
                    id: 2,
                    name: "sem cotação".to_string(),
                    unit_value: Decimal::ZERO,
                },
            ],
            transactions: vec![transaction(); 25],
            // Mais transações no total do que a página devolveu: há próxima.
            transaction_count: 27,
            snapshots: vec![snapshot(dec!(100)), snapshot(dec!(130))],
            ..FakeRepository::default()
        };

        let service = PortfolioService::new(fake);
        let view = service.wallet_view(1, 1).await.expect("wallet view");

        assert_eq!(view.summary.total_value, dec!(100));
        assert_eq!(view.holdings.len(), 1);
        // O catálogo administrativo pode conter ativos ainda sem preço, mas a
        // tela de compra só oferece os negociáveis.
        assert_eq!(view.available_assets.len(), 1);
        assert_eq!(view.transactions.len(), 25);
        assert_eq!(view.page, 1);
        assert!(!view.has_prev);
        assert!(view.has_next);
        assert!(view.chart.has_data());
        assert_eq!(view.chart.latest_value, dec!(130));
    }

    #[tokio::test]
    async fn deposit_result_flows_through_unchanged() {
        let service = PortfolioService::new(FakeRepository::default());
        assert!(service.deposit(1, dec!(50)).await.is_ok());
    }

    #[tokio::test]
    async fn buy_error_flows_through_unchanged() {
        let fake = FakeRepository {
            buy_result: RefCell::new(Err(AppError::InsufficientBalance)),
            ..FakeRepository::default()
        };
        let service = PortfolioService::new(fake);

        let result = service.buy(1, 1, dec!(1)).await;
        assert!(matches!(result, Err(AppError::InsufficientBalance)));
    }

    #[tokio::test]
    async fn sell_error_flows_through_unchanged() {
        let fake = FakeRepository {
            sell_result: RefCell::new(Err(AppError::InsufficientHoldings)),
            ..FakeRepository::default()
        };
        let service = PortfolioService::new(fake);

        let result = service.sell(1, 1, dec!(1)).await;
        assert!(matches!(result, Err(AppError::InsufficientHoldings)));
    }
}
