use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::admin::Admin;
use crate::error::AppError;
use crate::models::Asset;

/// Monta as rotas da API. O estado ainda não é fornecido aqui — só declaramos,
/// pelo tipo de retorno `Router<AppState>`, que este router espera um estado.
pub fn router() -> Router<AppState> {
    Router::new().route(
        "/assets",
        get(list_assets).post(create_asset).patch(update_asset),
    )
}

/// Lista todos os ativos cadastrados.
#[instrument(skip_all)]
async fn list_assets(State(state): State<AppState>) -> Json<Vec<Asset>> {
    let assets = state.assets.lock().await;
    Json(assets.values().cloned().collect())
}

#[derive(Deserialize)]
struct CreateAssetRequest {
    name: String,
    unit_value: f64,
}

/// Cadastra um novo ativo. Protegido: exige o `Admin`.
#[instrument(skip_all)]
async fn create_asset(
    State(state): State<AppState>,
    _admin: Admin,
    Json(request): Json<CreateAssetRequest>,
) -> Json<Asset> {
    let mut assets = state.assets.lock().await;

    // Sem banco de dados ainda: o próximo id é o maior existente + 1
    // (defaulta para 0 + 1 quando não há nenhum ativo).
    let id = assets
        .values()
        .map(|asset| asset.id)
        .max()
        .unwrap_or_default()
        + 1;

    let new_asset = Asset {
        id,
        name: request.name,
        unit_value: request.unit_value,
    };

    assets.insert(new_asset.id, new_asset.clone());

    Json(new_asset)
}

#[derive(Deserialize)]
struct UpdateAssetRequest {
    id: i64,
    name: Option<String>,
    unit_value: Option<f64>,
}

/// Atualiza um ativo existente. Protegido: exige o `Admin`. Nome e valor são
/// opcionais — só os campos enviados são alterados.
#[instrument(skip_all)]
async fn update_asset(
    State(state): State<AppState>,
    _admin: Admin,
    Json(request): Json<UpdateAssetRequest>,
) -> Result<Json<Asset>, AppError> {
    let mut assets = state.assets.lock().await;

    let existing_asset = assets
        .get_mut(&request.id)
        .ok_or(AppError::AssetDoesNotExist)?;

    if let Some(new_name) = request.name {
        existing_asset.name = new_name;
    }

    if let Some(new_unit_value) = request.unit_value {
        existing_asset.unit_value = new_unit_value;
    }

    Ok(Json(existing_asset.clone()))
}
