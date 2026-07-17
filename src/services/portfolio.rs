use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use rust_decimal::Decimal;

use crate::app::AppState;
use crate::error::AppError;
use crate::models::{Asset, Holding, Transaction, WalletSummary};
use crate::repository::Repository;

/// Transações por página do extrato.
pub const TRANSACTIONS_PAGE_SIZE: i64 = 25;

/// Tudo o que a tela da carteira precisa, montado numa chamada só.
pub struct WalletView {
    pub summary: WalletSummary,
    pub holdings: Vec<Holding>,
    pub available_assets: Vec<Asset>,
    pub transactions: Vec<Transaction>,
    pub page: u32,
    pub has_prev: bool,
    pub has_next: bool,
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

        let (summary, holdings, available_assets, transactions, total_transactions) = tokio::try_join!(
            self.repository.wallet_summary(user_id),
            self.repository.list_holdings(user_id),
            self.repository.list_assets(),
            self.repository
                .list_transactions(user_id, TRANSACTIONS_PAGE_SIZE, offset),
            self.repository.count_transactions(user_id)
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
