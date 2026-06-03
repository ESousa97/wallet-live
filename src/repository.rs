use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sqlx::PgPool;

use crate::app::AppState;
use crate::models::{Asset, UserRecord};

/// Camada que encapsula todo o acesso ao banco (padrão repository). Quem usa o
/// repository não precisa saber como o banco funciona — só que ele existe. Se o
/// formato dos dados ou as queries mudarem, basta alterar aqui.
pub struct Repository {
    db: PgPool,
}

/// O repository também é injetado pelos extratores do Axum, então os endpoints
/// não precisam pegar o `db` do estado manualmente. Nunca falha desde que o
/// estado esteja inicializado, daí o `Infallible`.
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
        sqlx::query_as!(Asset, "SELECT id, name, unit_value FROM assets")
            .fetch_all(&self.db)
            .await
    }

    pub async fn create_asset(&self, name: String, unit_value: f64) -> sqlx::Result<Asset> {
        // O id é criado pelo próprio banco (BIGSERIAL); pedimos o registro de
        // volta com RETURNING para reconstruir o Asset.
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
        unit_value: Option<f64>,
    ) -> sqlx::Result<Option<Asset>> {
        // COALESCE mantém o valor atual quando o parâmetro vem nulo (None).
        // fetch_optional devolve None quando o id não existe.
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

    /// Insere um novo usuário e devolve o registro recém-criado. Recebe a
    /// `password_hash` já pronta: a camada de repository não precisa saber como a
    /// senha é hasheada — confiamos que o módulo de autenticação entregou a hash.
    /// O `id` é gerado pelo banco (BIGSERIAL) e volta via RETURNING.
    pub async fn add_user(
        &self,
        username: String,
        password_hash: String,
    ) -> sqlx::Result<UserRecord> {
        sqlx::query_as!(
            UserRecord,
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id, username, password_hash",
            username,
            password_hash
        )
        .fetch_one(&self.db)
        .await
    }

    /// Busca um usuário pelo nome (a chave de login). Devolve `Option` porque o
    /// usuário pode simplesmente não existir — é mais fácil tratar isso do que
    /// inspecionar o erro `RowNotFound` do SQLx, e modela melhor a realidade.
    pub async fn get_user_by_name(&self, username: &str) -> sqlx::Result<Option<UserRecord>> {
        sqlx::query_as!(
            UserRecord,
            "SELECT id, username, password_hash FROM users WHERE username = $1",
            username
        )
        .fetch_optional(&self.db)
        .await
    }
}

/// Só nos testes: permite converter uma `PgPool` direto num `Repository` sem
/// precisar expor o campo `db` (que deve continuar privado à camada repository).
#[cfg(test)]
impl From<PgPool> for Repository {
    fn from(db: PgPool) -> Self {
        Self { db }
    }
}
