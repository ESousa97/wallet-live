use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
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
    #[error("user does not exist")]
    UserDoesNotExist,
    #[error("username already taken")]
    UsernameTaken,
    #[error("invalid amount")]
    InvalidAmount,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("insufficient holdings")]
    InsufficientHoldings,
    #[error("market quote unavailable")]
    QuoteUnavailable,
    // `transparent` delega Display/source ao erro interno; `#[from]` gera a
    // conversão automática, então um erro de SQLx vira AppError com `?`.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    // Falha ao renderizar um template Askama — provavelmente arquivo ausente ou
    // mal formatado: erro de configuração nosso, não do cliente.
    #[error(transparent)]
    Template(#[from] askama::Error),
    // Falha ao gerar/validar um JWT (token fabricado, expirado ou com assinatura
    // inválida). Guardamos só a mensagem porque `jwt_simple::Error` é um
    // `anyhow::Error` (não implementa `std::error::Error`, então não dá pra usar
    // `#[from]`/`transparent`); a conversão fica no `From` manual abaixo.
    #[error("token error: {0}")]
    Jwt(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

impl From<jwt_simple::Error> for AppError {
    fn from(error: jwt_simple::Error) -> Self {
        AppError::Jwt(error.to_string())
    }
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
            AppError::UserDoesNotExist => StatusCode::NOT_FOUND,
            // O nome já está em uso: erro do cliente.
            AppError::UsernameTaken => StatusCode::BAD_REQUEST,
            AppError::InvalidAmount => StatusCode::BAD_REQUEST,
            AppError::InsufficientBalance => StatusCode::BAD_REQUEST,
            AppError::InsufficientHoldings => StatusCode::BAD_REQUEST,
            AppError::QuoteUnavailable => StatusCode::BAD_GATEWAY,
            // Algo inesperado aconteceu no banco: erro interno do servidor.
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            // Falha ao renderizar template: configuração nossa, erro interno.
            AppError::Template(_) => StatusCode::INTERNAL_SERVER_ERROR,
            // Token ausente/inválido nas rotas que exigem `User` diretamente.
            AppError::Jwt(_) => StatusCode::UNAUTHORIZED,
            AppError::Http(_) => StatusCode::BAD_GATEWAY,
        };

        (status, Json(error_response)).into_response()
    }
}
