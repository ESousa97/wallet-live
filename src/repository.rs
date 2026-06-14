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

    pub async fn update_known_asset_prices(
        &self,
        updates: &HashMap<&str, Decimal>,
    ) -> Result<usize, AppError> {
        let assets = self.list_assets().await?;
        let mut updated = 0;

        for asset in assets {
            let key = asset.name.trim().to_lowercase();

            if let Some(unit_value) = updates.get(key.as_str()) {
                self.update_asset(asset.id, None, Some(*unit_value)).await?;
                updated += 1;
            }
        }

        Ok(updated)
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
