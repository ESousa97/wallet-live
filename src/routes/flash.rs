use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::error::AppError;
use crate::i18n::Strings;

/// Nome do cookie que transporta a mensagem entre o POST e a página seguinte.
const FLASH_COOKIE: &str = "flash";

/// Mensagem de feedback de uma operação, exibida UMA vez na próxima página
/// (padrão *flash message*): o POST grava o cookie e redireciona; o GET lê,
/// mostra o banner e remove o cookie.
pub struct Flash {
    pub message: String,
    error: bool,
}

impl Flash {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error: true,
        }
    }

    pub fn is_error(&self) -> bool {
        self.error
    }
}

/// Grava o flash na jar. O texto vai em base64 (acentos sobrevivem ao cookie) e
/// o cookie dura só um minuto — se nunca for lido, o navegador o descarta.
pub fn set_flash(jar: CookieJar, flash: &Flash, cookie_secure: bool) -> CookieJar {
    let kind = if flash.error { 'e' } else { 's' };
    let value = format!("{kind}:{}", URL_SAFE_NO_PAD.encode(&flash.message));

    jar.add(
        Cookie::build((FLASH_COOKIE, value))
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(cookie_secure)
            .path("/")
            .max_age(time::Duration::minutes(1))
            .build(),
    )
}

/// Lê e REMOVE o flash da jar (flash é de uso único). Valor malformado é
/// simplesmente descartado.
pub fn take_flash(jar: CookieJar) -> (CookieJar, Option<Flash>) {
    let Some(cookie) = jar.get(FLASH_COOKIE) else {
        return (jar, None);
    };

    let flash = cookie.value().split_once(':').and_then(|(kind, encoded)| {
        let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
        let message = String::from_utf8(bytes).ok()?;
        match kind {
            "s" => Some(Flash::success(message)),
            "e" => Some(Flash::error(message)),
            _ => None,
        }
    });

    (
        jar.remove(Cookie::build(FLASH_COOKIE).path("/").build()),
        flash,
    )
}

/// Traduz erros DE NEGÓCIO em mensagens amigáveis no idioma da requisição para
/// o banner. Erros internos (banco, template, JWT quebrado) NÃO viram flash:
/// são devolvidos e seguem o fluxo normal de erro (500 logado, resposta
/// genérica).
///
/// Detalhe deliberado: credencial inválida e usuário inexistente têm a MESMA
/// mensagem — a tela de login não confirma se um username existe.
pub fn business_flash(error: AppError, t: &Strings) -> Result<Flash, AppError> {
    let message = match &error {
        AppError::InvalidAmount => t.flash_invalid_amount,
        AppError::InsufficientBalance => t.flash_insufficient_balance,
        AppError::InsufficientHoldings => t.flash_insufficient_holdings,
        AppError::AssetDoesNotExist => t.flash_asset_missing,
        AppError::InvalidCredentials | AppError::UserDoesNotExist => t.flash_bad_credentials,
        AppError::UsernameTaken => t.flash_username_taken,
        AppError::TooManyAttempts => t.flash_too_many_attempts,
        AppError::CsrfMismatch => t.flash_csrf,
        AppError::QuoteUnavailable => t.flash_quotes_unavailable,
        _ => return Err(error),
    };

    Ok(Flash::error(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_roundtrips_through_the_cookie_including_accents() {
        let jar = set_flash(
            CookieJar::new(),
            &Flash::error("posição insuficiente para esta venda."),
            false,
        );

        let (jar, flash) = take_flash(jar);
        let flash = flash.expect("flash present");
        assert!(flash.is_error());
        assert_eq!(flash.message, "posição insuficiente para esta venda.");

        // Flash é de uso único: a segunda leitura vem vazia.
        let (_, again) = take_flash(jar);
        assert!(again.is_none());
    }

    #[test]
    fn success_kind_survives_the_roundtrip() {
        let jar = set_flash(
            CookieJar::new(),
            &Flash::success("depósito realizado."),
            false,
        );
        let (_, flash) = take_flash(jar);
        assert!(!flash.expect("flash").is_error());
    }

    #[test]
    fn business_errors_become_messages_and_internal_errors_do_not() {
        use crate::i18n::{EN, PT_BR};

        assert!(business_flash(AppError::InsufficientBalance, &PT_BR).is_ok());
        // Mesma mensagem para credencial e usuário inexistente (não vaza
        // existência de conta) — nos dois idiomas.
        for t in [&PT_BR, &EN] {
            let a = business_flash(AppError::InvalidCredentials, t)
                .unwrap()
                .message;
            let b = business_flash(AppError::UserDoesNotExist, t)
                .unwrap()
                .message;
            assert_eq!(a, b);
        }
        // O idioma muda a mensagem de fato.
        assert_ne!(
            business_flash(AppError::InsufficientBalance, &PT_BR)
                .unwrap()
                .message,
            business_flash(AppError::InsufficientBalance, &EN)
                .unwrap()
                .message,
        );

        // Erro interno passa direto para o fluxo de 500.
        assert!(business_flash(AppError::Jwt("boom".into()), &PT_BR).is_err());
    }
}
