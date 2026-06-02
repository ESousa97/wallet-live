use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::app::AppState;
use crate::error::AppError;

/// Chave secreta do admin. Em produção viria de uma variável de ambiente ou de
/// um cofre de segredos (ex.: AWS Secrets Manager).
const ADMIN_SECRET_KEY: &str = "I'm the admin";

/// Extrator que só é construído com sucesso se a requisição trouxer a secret
/// key correta no header `Authorization`. Como o Axum exige construir todos os
/// extratores antes de chamar o handler, basta anotar um endpoint com um
/// parâmetro do tipo `Admin` para protegê-lo: sem credencial válida, o código
/// do handler nem chega a rodar.
pub struct Admin;

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or(AppError::MissingAuthorization)?;

        if authorization == ADMIN_SECRET_KEY {
            Ok(Admin)
        } else {
            Err(AppError::InvalidCredentials)
        }
    }
}
