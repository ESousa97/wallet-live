use axum::routing::get;
use axum::{Json, Router};
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::admin::Admin;
use crate::error::AppError;
use crate::models::Asset;
use crate::repository::Repository;

/// Monta as rotas da API. O router ainda espera um `AppState` (de onde o
/// `Repository` é extraído).
pub fn router() -> Router<AppState> {
    Router::new().route(
        "/assets",
        get(list_assets).post(create_asset).patch(update_asset),
    )
}

/// Lista todos os ativos.
#[instrument(skip_all)]
async fn list_assets(repository: Repository) -> Result<Json<Vec<Asset>>, AppError> {
    let assets = repository.list_assets().await?;
    Ok(Json(assets))
}

#[derive(Deserialize)]
struct CreateAssetRequest {
    name: String,
    unit_value: Decimal,
}

/// Cadastra um novo ativo. Protegido: exige o `Admin`.
#[instrument(skip_all)]
async fn create_asset(
    _admin: Admin,
    repository: Repository,
    Json(request): Json<CreateAssetRequest>,
) -> Result<Json<Asset>, AppError> {
    let new_asset = repository
        .create_asset(request.name, request.unit_value)
        .await?;
    Ok(Json(new_asset))
}

#[derive(Deserialize)]
struct UpdateAssetRequest {
    id: i64,
    name: Option<String>,
    unit_value: Option<Decimal>,
}

/// Atualiza um ativo existente. Protegido: exige o `Admin`. Nome e valor são
/// opcionais — só os campos enviados são alterados.
#[instrument(skip_all)]
async fn update_asset(
    _admin: Admin,
    repository: Repository,
    Json(request): Json<UpdateAssetRequest>,
) -> Result<Json<Asset>, AppError> {
    let updated_asset = repository
        .update_asset(request.id, request.name, request.unit_value)
        .await?;

    match updated_asset {
        Some(updated_asset) => Ok(Json(updated_asset)),
        None => Err(AppError::AssetDoesNotExist),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[sqlx::test]
    async fn test_create_asset(db: sqlx::PgPool) {
        let request = CreateAssetRequest {
            name: "bitcoin".to_string(),
            unit_value: dec!(10),
        };

        let Json(new_asset) = create_asset(Admin, db.into(), Json(request))
            .await
            .expect("success");

        assert_eq!(new_asset.id, 1);
        assert_eq!(new_asset.name, "bitcoin");
        assert_eq!(new_asset.unit_value, dec!(10));

        insta::assert_json_snapshot!(new_asset);
    }

    #[sqlx::test(fixtures("bitcoin_asset"))]
    async fn test_list_assets(db: sqlx::PgPool) {
        let Json(assets) = list_assets(db.into()).await.expect("success");

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].name, "bitcoin");

        insta::assert_json_snapshot!(assets);
    }

    #[sqlx::test(fixtures("bitcoin_asset"))]
    async fn test_update_asset(db: sqlx::PgPool) {
        let request = UpdateAssetRequest {
            id: 1,
            name: Some("ethereum".to_string()),
            unit_value: Some(dec!(20)),
        };

        let Json(updated_asset) = update_asset(Admin, db.into(), Json(request))
            .await
            .expect("success");

        assert_eq!(updated_asset.id, 1);
        assert_eq!(updated_asset.name, "ethereum");
        assert_eq!(updated_asset.unit_value, dec!(20));

        insta::assert_json_snapshot!(updated_asset);
    }
}
