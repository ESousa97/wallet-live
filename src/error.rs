use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// Todos os erros possíveis da aplicação reunidos num único enum.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("missing authorization header")]
    MissingAuthorization,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("asset does not exist")]
    AssetDoesNotExist,
    // `transparent` delega Display/source ao erro interno; `#[from]` gera a
    // conversão automática, então um erro de SQLx vira AppError com `?`.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Formato JSON de um erro devolvido pela API.
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_response = ErrorResponse {
            error: self.to_string(),
        };

        // Cada erro vira um status HTTP apropriado, em vez de devolver 200.
        let status = match self {
            AppError::MissingAuthorization => StatusCode::BAD_REQUEST,
            AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::AssetDoesNotExist => StatusCode::NOT_FOUND,
            // Algo inesperado aconteceu no banco: erro interno do servidor.
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(error_response)).into_response()
    }
}
