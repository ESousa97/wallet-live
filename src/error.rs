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
    #[error("username or password does not meet the registration requirements")]
    InvalidRegistration,
    #[error("invalid amount")]
    InvalidAmount,
    #[error("trade total is below the supported monetary precision")]
    TradeTooSmall,
    #[error("too many failed attempts, try again later")]
    TooManyAttempts,
    #[error("invalid csrf token")]
    CsrfMismatch,
    #[error("asset name must not be empty")]
    InvalidAssetName,
    #[error("unit value must not be negative")]
    NegativeUnitValue,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("insufficient holdings")]
    InsufficientHoldings,
    #[error("market quote unavailable")]
    QuoteUnavailable,
    #[error("quotes were refreshed recently")]
    QuoteSyncTooSoon,
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
    // Resposta de terceiro que não casa com o formato que esperamos: campo
    // ausente, tipo trocado, número que não cabe no `Decimal`. É falha DELES,
    // não nossa, e por isso responde 502 como qualquer indisponibilidade da
    // fonte — mas com a mensagem do serde no log, que diz linha e coluna do
    // corpo. Ter esta variante separada de `Http` é o que permite decodificar
    // um payload sem rede (ver `market::parse_markets` e
    // `quotes::parse_brl_rates`, que a suíte de `tests/` atravessa com as
    // respostas reais versionadas em `tests/payloads/`).
    #[error("upstream payload does not match the expected shape: {0}")]
    Payload(#[from] serde_json::Error),
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
        // Cada erro vira um status HTTP apropriado, em vez de devolver 200.
        // Casamos por referência para ainda poder usar `self` no log/mensagem.
        let status = match &self {
            AppError::MissingAuthorization => StatusCode::BAD_REQUEST,
            AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::AssetDoesNotExist => StatusCode::NOT_FOUND,
            AppError::UserDoesNotExist => StatusCode::NOT_FOUND,
            // O nome já está em uso: erro do cliente.
            AppError::UsernameTaken => StatusCode::BAD_REQUEST,
            AppError::InvalidRegistration => StatusCode::BAD_REQUEST,
            AppError::InvalidAmount => StatusCode::BAD_REQUEST,
            AppError::TradeTooSmall => StatusCode::BAD_REQUEST,
            // Lockout de força bruta no login.
            AppError::TooManyAttempts => StatusCode::TOO_MANY_REQUESTS,
            // Token CSRF ausente ou divergente: a requisição não veio de um
            // formulário que nós renderizamos.
            AppError::CsrfMismatch => StatusCode::FORBIDDEN,
            AppError::InvalidAssetName => StatusCode::BAD_REQUEST,
            AppError::NegativeUnitValue => StatusCode::BAD_REQUEST,
            AppError::InsufficientBalance => StatusCode::BAD_REQUEST,
            AppError::InsufficientHoldings => StatusCode::BAD_REQUEST,
            AppError::QuoteUnavailable => StatusCode::BAD_GATEWAY,
            AppError::QuoteSyncTooSoon => StatusCode::TOO_MANY_REQUESTS,
            // Algo inesperado aconteceu no banco: erro interno do servidor.
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            // Falha ao renderizar template: configuração nossa, erro interno.
            AppError::Template(_) => StatusCode::INTERNAL_SERVER_ERROR,
            // Token ausente/inválido nas rotas que exigem `User` diretamente.
            AppError::Jwt(_) => StatusCode::UNAUTHORIZED,
            AppError::Http(_) => StatusCode::BAD_GATEWAY,
            AppError::Payload(_) => StatusCode::BAD_GATEWAY,
        };

        // Falhas 5xx são problema NOSSO: registramos o erro completo no servidor
        // (com a causa raiz) e devolvemos uma mensagem genérica ao cliente. Assim
        // não vazamos detalhes internos — texto de erro do SQL, nomes de colunas,
        // string de conexão — na resposta HTTP.
        let error = if status.is_server_error() {
            tracing::error!(error = ?self, "internal error serving request");
            "internal server error".to_string()
        } else {
            self.to_string()
        };

        (status, Json(ErrorResponse { error })).into_response()
    }
}
