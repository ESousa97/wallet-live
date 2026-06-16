use std::collections::HashMap;
use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres, Transaction as SqlxTransaction};

use crate::app::AppState;
use crate::error::AppError;
use crate::models::{Asset, Holding, Transaction, UserRecord, WalletSummary};

pub struct Repository {
    db: PgPool,
}

impl FromRequestParts<AppState> for Repository {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self {
            db: state.db.clone(),
        })
    }
}

impl Repository {
    pub async fn list_assets(&self) -> sqlx::Result<Vec<Asset>> {
        sqlx::query_as!(Asset, "SELECT id, name, unit_value FROM assets ORDER BY id")
            .fetch_all(&self.db)
            .await
    }

    pub async fn create_asset(&self, name: String, unit_value: Decimal) -> sqlx::Result<Asset> {
        sqlx::query_as!(
            Asset,
            "INSERT INTO assets (name, unit_value) VALUES ($1, $2) RETURNING id, name, unit_value",
            name,
            unit_value
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn update_asset(
        &self,
        asset_id: i64,
        name: Option<String>,
        unit_value: Option<Decimal>,
    ) -> sqlx::Result<Option<Asset>> {
        sqlx::query_as!(
            Asset,
            "UPDATE assets SET name = COALESCE($2, name), unit_value = COALESCE($3, unit_value) WHERE id = $1 RETURNING id, name, unit_value",
            asset_id,
            name,
            unit_value
        )
        .fetch_optional(&self.db)
        .await
    }

    /// Aplica novos preços a ativos identificados pelo nome (normalizado para
    /// minúsculas, sem espaços nas pontas). Faz tudo em **uma** ida ao banco:
    /// `UNNEST` transforma os dois arrays (nomes e valores) numa tabela virtual e
    /// o `UPDATE ... FROM` casa pelo nome. Antes era um `SELECT` seguido de um
    /// `UPDATE` por ativo (N+1) — agora é um statement só.
    pub async fn update_known_asset_prices(
        &self,
        updates: &HashMap<&str, Decimal>,
    ) -> Result<usize, AppError> {
        // `unzip` garante que `names[i]` corresponde a `values[i]` (iterar
        // `keys()` e `values()` em separado não daria essa garantia).
        let (names, values): (Vec<String>, Vec<Decimal>) = updates
            .iter()
            .map(|(name, value)| (name.to_string(), *value))
            .unzip();

        let result = sqlx::query!(
            r#"
            UPDATE assets AS a
            SET unit_value = u.value
            FROM UNNEST($1::text[], $2::numeric[]) AS u(name, value)
            WHERE LOWER(TRIM(a.name)) = u.name
            "#,
            &names,
            &values
        )
        .execute(&self.db)
        .await?;

        Ok(result.rows_affected() as usize)
    }

    pub async fn add_user(
        &self,
        username: String,
        password_hash: String,
    ) -> sqlx::Result<UserRecord> {
        sqlx::query_as!(
            UserRecord,
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id, username, password_hash, balance",
            username,
            password_hash
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn get_user_by_name(&self, username: &str) -> sqlx::Result<Option<UserRecord>> {
        sqlx::query_as!(
            UserRecord,
            "SELECT id, username, password_hash, balance FROM users WHERE username = $1",
            username
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn wallet_summary(&self, user_id: i64) -> sqlx::Result<WalletSummary> {
        sqlx::query_as!(
            WalletSummary,
            r#"
            SELECT
                u.balance,
                COALESCE(SUM(h.quantity * a.unit_value), 0) AS "holdings_value!",
                u.balance + COALESCE(SUM(h.quantity * a.unit_value), 0) AS "total_value!",
                COALESCE(SUM(h.quantity * h.avg_cost), 0) AS "total_invested!",
                COALESCE(SUM(h.quantity * (a.unit_value - h.avg_cost)), 0) AS "total_delta!"
            FROM users u
            LEFT JOIN holdings h ON h.user_id = u.id
            LEFT JOIN assets a ON a.id = h.asset_id
            WHERE u.id = $1
            GROUP BY u.id, u.balance
            "#,
            user_id
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn list_holdings(&self, user_id: i64) -> sqlx::Result<Vec<Holding>> {
        sqlx::query_as!(
            Holding,
            r#"
            SELECT
                a.id,
                a.name,
                a.unit_value,
                h.quantity AS "quantity_owned!",
                h.avg_cost,
                h.quantity * a.unit_value AS "current_value!",
                h.quantity * h.avg_cost AS "invested_value!",
                h.quantity * (a.unit_value - h.avg_cost) AS "value_delta!"
            FROM holdings h
            JOIN assets a ON a.id = h.asset_id
            WHERE h.user_id = $1
            ORDER BY a.id
            "#,
            user_id
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn list_transactions(&self, user_id: i64) -> sqlx::Result<Vec<Transaction>> {
        sqlx::query_as!(
            Transaction,
            r#"
            SELECT
                t.id,
                t.kind,
                a.name AS "asset_name?",
                t.quantity,
                t.unit_value,
                t.cash_delta,
                t.created_at
            FROM transactions t
            LEFT JOIN assets a ON a.id = t.asset_id
            WHERE t.user_id = $1
            ORDER BY t.created_at DESC, t.id DESC
            LIMIT 25
            "#,
            user_id
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn deposit(&self, user_id: i64, amount: Decimal) -> Result<(), AppError> {
        if amount <= Decimal::ZERO {
            return Err(AppError::InvalidAmount);
        }

        let mut tx = self.db.begin().await?;

        sqlx::query!(
            "UPDATE users SET balance = balance + $2 WHERE id = $1",
            user_id,
            amount
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO transactions (user_id, kind, cash_delta) VALUES ($1, 'deposit', $2)",
            user_id,
            amount
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn buy_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError> {
        if quantity <= Decimal::ZERO {
            return Err(AppError::InvalidAmount);
        }

        let mut tx = self.db.begin().await?;
        let asset = asset_for_update(&mut tx, asset_id).await?;
        let cost = asset.unit_value * quantity;

        let user = sqlx::query!(
            "SELECT balance FROM users WHERE id = $1 FOR UPDATE",
            user_id
        )
        .fetch_one(&mut *tx)
        .await?;

        if user.balance < cost {
            return Err(AppError::InsufficientBalance);
        }

        sqlx::query!(
            "UPDATE users SET balance = balance - $2 WHERE id = $1",
            user_id,
            cost
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO holdings (user_id, asset_id, quantity, avg_cost)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id, asset_id) DO UPDATE SET
                avg_cost = ((holdings.quantity * holdings.avg_cost) + (EXCLUDED.quantity * EXCLUDED.avg_cost))
                    / (holdings.quantity + EXCLUDED.quantity),
                quantity = holdings.quantity + EXCLUDED.quantity
            "#,
            user_id,
            asset_id,
            quantity,
            asset.unit_value
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO transactions (user_id, kind, asset_id, quantity, unit_value, cash_delta) VALUES ($1, 'buy', $2, $3, $4, $5)",
            user_id,
            asset_id,
            quantity,
            asset.unit_value,
            -cost
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn sell_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: Decimal,
    ) -> Result<(), AppError> {
        if quantity <= Decimal::ZERO {
            return Err(AppError::InvalidAmount);
        }

        let mut tx = self.db.begin().await?;
        let asset = asset_for_update(&mut tx, asset_id).await?;

        let holding = sqlx::query!(
            "SELECT quantity FROM holdings WHERE user_id = $1 AND asset_id = $2 FOR UPDATE",
            user_id,
            asset_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::InsufficientHoldings)?;

        if holding.quantity < quantity {
            return Err(AppError::InsufficientHoldings);
        }

        let proceeds = asset.unit_value * quantity;

        sqlx::query!(
            "UPDATE users SET balance = balance + $2 WHERE id = $1",
            user_id,
            proceeds
        )
        .execute(&mut *tx)
        .await?;

        if holding.quantity == quantity {
            sqlx::query!(
                "DELETE FROM holdings WHERE user_id = $1 AND asset_id = $2",
                user_id,
                asset_id
            )
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query!(
                "UPDATE holdings SET quantity = quantity - $3 WHERE user_id = $1 AND asset_id = $2",
                user_id,
                asset_id,
                quantity
            )
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query!(
            "INSERT INTO transactions (user_id, kind, asset_id, quantity, unit_value, cash_delta) VALUES ($1, 'sell', $2, $3, $4, $5)",
            user_id,
            asset_id,
            quantity,
            asset.unit_value,
            proceeds
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

async fn asset_for_update(
    tx: &mut SqlxTransaction<'_, Postgres>,
    asset_id: i64,
) -> Result<Asset, AppError> {
    sqlx::query_as!(
        Asset,
        "SELECT id, name, unit_value FROM assets WHERE id = $1 FOR UPDATE",
        asset_id
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AppError::AssetDoesNotExist)
}

#[cfg(test)]
impl From<PgPool> for Repository {
    fn from(db: PgPool) -> Self {
        Self { db }
    }
}

// Testes do núcleo financeiro: depósito, compra, venda, custo médio e as guardas
// de saldo/posição. É a parte do sistema que mexe em dinheiro — a que mais merece
// rede de proteção. Cada `#[sqlx::test]` roda num banco efêmero próprio (migrações
// aplicadas automaticamente), então os testes são isolados e podem rodar em
// paralelo.
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    async fn new_user(repo: &Repository, name: &str) -> i64 {
        repo.add_user(name.to_string(), "stub-hash".to_string())
            .await
            .expect("create user")
            .id
    }

    async fn new_asset(repo: &Repository, name: &str, price: Decimal) -> i64 {
        repo.create_asset(name.to_string(), price)
            .await
            .expect("create asset")
            .id
    }

    #[sqlx::test]
    async fn deposit_credits_balance_and_logs_transaction(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;

        repo.deposit(uid, dec!(100)).await.expect("deposit");

        let summary = repo.wallet_summary(uid).await.expect("summary");
        assert_eq!(summary.balance, dec!(100));

        let txs = repo.list_transactions(uid).await.expect("transactions");
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].kind, "deposit");
        assert_eq!(txs[0].cash_delta, dec!(100));
    }

    #[sqlx::test]
    async fn deposit_rejects_non_positive_amounts(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;

        assert!(matches!(
            repo.deposit(uid, dec!(0)).await,
            Err(AppError::InvalidAmount)
        ));
        assert!(matches!(
            repo.deposit(uid, dec!(-5)).await,
            Err(AppError::InvalidAmount)
        ));

        assert_eq!(repo.wallet_summary(uid).await.unwrap().balance, dec!(0));
    }

    #[sqlx::test]
    async fn buy_debits_balance_and_opens_holding(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;
        let aid = new_asset(&repo, "bitcoin", dec!(10)).await;
        repo.deposit(uid, dec!(100)).await.unwrap();

        repo.buy_asset(uid, aid, dec!(3)).await.expect("buy");

        let summary = repo.wallet_summary(uid).await.unwrap();
        assert_eq!(summary.balance, dec!(70));
        assert_eq!(summary.holdings_value, dec!(30));

        let holdings = repo.list_holdings(uid).await.unwrap();
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].quantity_owned, dec!(3));
        assert_eq!(holdings[0].avg_cost, dec!(10));
    }

    #[sqlx::test]
    async fn buy_rejects_when_balance_is_insufficient(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;
        let aid = new_asset(&repo, "bitcoin", dec!(10)).await;
        repo.deposit(uid, dec!(10)).await.unwrap();

        // Custo 20 (2 @ 10) maior que o saldo 10.
        let result = repo.buy_asset(uid, aid, dec!(2)).await;
        assert!(matches!(result, Err(AppError::InsufficientBalance)));

        // A transação inteira foi revertida: saldo intacto, nenhuma posição.
        assert_eq!(repo.wallet_summary(uid).await.unwrap().balance, dec!(10));
        assert!(repo.list_holdings(uid).await.unwrap().is_empty());
    }

    #[sqlx::test]
    async fn buy_rejects_unknown_asset(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;
        repo.deposit(uid, dec!(100)).await.unwrap();

        let result = repo.buy_asset(uid, 999, dec!(1)).await;
        assert!(matches!(result, Err(AppError::AssetDoesNotExist)));
    }

    #[sqlx::test]
    async fn buying_more_averages_the_cost_basis(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;
        let aid = new_asset(&repo, "bitcoin", dec!(10)).await;
        repo.deposit(uid, dec!(1000)).await.unwrap();

        repo.buy_asset(uid, aid, dec!(2)).await.unwrap(); // 2 @ 10
        repo.update_asset(aid, None, Some(dec!(20))).await.unwrap();
        repo.buy_asset(uid, aid, dec!(2)).await.unwrap(); // 2 @ 20

        let holdings = repo.list_holdings(uid).await.unwrap();
        assert_eq!(holdings[0].quantity_owned, dec!(4));
        // (2*10 + 2*20) / 4 = 15
        assert_eq!(holdings[0].avg_cost, dec!(15));
    }

    #[sqlx::test]
    async fn selling_everything_closes_the_position(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;
        let aid = new_asset(&repo, "bitcoin", dec!(10)).await;
        repo.deposit(uid, dec!(100)).await.unwrap();
        repo.buy_asset(uid, aid, dec!(3)).await.unwrap(); // saldo 70, 3 un

        repo.sell_asset(uid, aid, dec!(3)).await.expect("sell");

        assert!(repo.list_holdings(uid).await.unwrap().is_empty());
        // 70 + 3 * 10 de volta.
        assert_eq!(repo.wallet_summary(uid).await.unwrap().balance, dec!(100));
    }

    #[sqlx::test]
    async fn partial_sell_keeps_remaining_units(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;
        let aid = new_asset(&repo, "bitcoin", dec!(10)).await;
        repo.deposit(uid, dec!(100)).await.unwrap();
        repo.buy_asset(uid, aid, dec!(4)).await.unwrap(); // saldo 60

        repo.sell_asset(uid, aid, dec!(1)).await.unwrap(); // +10 -> 70

        let holdings = repo.list_holdings(uid).await.unwrap();
        assert_eq!(holdings[0].quantity_owned, dec!(3));
        // Vender não altera o custo médio das unidades restantes.
        assert_eq!(holdings[0].avg_cost, dec!(10));
        assert_eq!(repo.wallet_summary(uid).await.unwrap().balance, dec!(70));
    }

    #[sqlx::test]
    async fn sell_rejects_more_than_owned(db: PgPool) {
        let repo = Repository::from(db);
        let uid = new_user(&repo, "alice").await;
        let aid = new_asset(&repo, "bitcoin", dec!(10)).await;
        repo.deposit(uid, dec!(100)).await.unwrap();
        repo.buy_asset(uid, aid, dec!(1)).await.unwrap();

        // Mais do que a posição comporta.
        assert!(matches!(
            repo.sell_asset(uid, aid, dec!(2)).await,
            Err(AppError::InsufficientHoldings)
        ));

        // Vender um ativo sem nenhuma posição também é rejeitado.
        let other = new_asset(&repo, "ethereum", dec!(5)).await;
        assert!(matches!(
            repo.sell_asset(uid, other, dec!(1)).await,
            Err(AppError::InsufficientHoldings)
        ));
    }

    #[sqlx::test]
    async fn update_known_asset_prices_matches_by_normalized_name(db: PgPool) {
        let repo = Repository::from(db);
        // "Bitcoin" com maiúscula deve casar com a chave "bitcoin" (normalização).
        let btc = new_asset(&repo, "Bitcoin", dec!(1)).await;
        let real = new_asset(&repo, "real", dec!(1)).await;
        let eth = new_asset(&repo, "ethereum", dec!(7)).await;

        let mut updates = HashMap::new();
        updates.insert("bitcoin", dec!(500000));
        updates.insert("real", dec!(1));
        updates.insert("dolar", dec!(5)); // sem ativo correspondente: ignorado

        let count = repo
            .update_known_asset_prices(&updates)
            .await
            .expect("update prices");
        assert_eq!(count, 2);

        let assets = repo.list_assets().await.unwrap();
        let price_of = |id: i64| assets.iter().find(|a| a.id == id).unwrap().unit_value;
        assert_eq!(price_of(btc), dec!(500000));
        assert_eq!(price_of(real), dec!(1));
        assert_eq!(price_of(eth), dec!(7)); // intocado
    }
}
