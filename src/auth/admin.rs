use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use subtle::ConstantTimeEq;

use crate::app::AppState;
use crate::error::AppError;

/// Extrator que só é construído com sucesso se a requisição trouxer a secret
/// key correta no header `Authorization`. Como o Axum exige construir todos os
/// extratores antes de chamar o handler, basta anotar um endpoint com um
/// parâmetro do tipo `Admin` para protegê-lo: sem credencial válida, o código
/// do handler nem chega a rodar.
///
/// A chave esperada vem da `Config` (lida do ambiente na inicialização, ver
/// `config.rs`) — não é mais relida do ambiente a cada requisição.
pub struct Admin;

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or(AppError::MissingAuthorization)?;

        let provided = authorization
            .to_str()
            .map_err(|_| AppError::InvalidCredentials)?;

        // `ct_eq` compara em tempo constante: não vaza, pelo tempo de resposta,
        // quantos bytes do segredo bateram antes de divergir.
        if provided
            .as_bytes()
            .ct_eq(state.config.admin_secret_key.as_bytes())
            .into()
        {
            Ok(Admin)
        } else {
            Err(AppError::InvalidCredentials)
        }
    }
}
