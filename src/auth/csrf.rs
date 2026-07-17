use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use subtle::ConstantTimeEq;

use crate::error::AppError;

/// Nome do cookie que guarda o token anti-CSRF.
pub const CSRF_COOKIE: &str = "csrf";

/// Proteção CSRF no padrão *double-submit cookie*: o servidor gera um token
/// aleatório, grava-o num cookie E embute o mesmo valor num campo oculto do
/// formulário. No POST, os dois têm de bater. Um site malicioso consegue fazer
/// o navegador ENVIAR os cookies da vítima, mas não consegue LER o cookie para
/// preencher o campo do formulário — então a requisição forjada falha.
///
/// O `SameSite=Strict` do cookie de sessão já bloqueia a maior parte dos
/// ataques CSRF em navegadores modernos; isto aqui é defesa em profundidade
/// (navegadores antigos, brechas de same-site, subdomínios comprometidos).
///
/// Garante que a jar tem um token CSRF, reutilizando o existente ou gerando um
/// novo. Retorna a jar (possivelmente com o cookie novo) e o valor do token,
/// para o handler embutir no formulário renderizado.
pub fn ensure_csrf_token(jar: CookieJar, cookie_secure: bool) -> (CookieJar, String) {
    if let Some(cookie) = jar.get(CSRF_COOKIE) {
        let token = cookie.value().to_string();
        return (jar, token);
    }

    let token = random_token();
    let cookie = Cookie::build((CSRF_COOKIE, token.clone()))
        // O template recebe o token pelo servidor, não por JavaScript — então o
        // cookie pode (e deve) ficar inacessível a scripts.
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(cookie_secure)
        .path("/")
        .build();

    (jar.add(cookie), token)
}

/// Confere o token submetido no formulário contra o cookie. Comparação em tempo
/// constante, como toda comparação de segredo.
pub fn verify_csrf(jar: &CookieJar, submitted: &str) -> Result<(), AppError> {
    let cookie = jar.get(CSRF_COOKIE).ok_or(AppError::CsrfMismatch)?;

    if submitted.as_bytes().ct_eq(cookie.value().as_bytes()).into() {
        Ok(())
    } else {
        Err(AppError::CsrfMismatch)
    }
}

/// 32 bytes de aleatoriedade do SO, codificados em base64 url-safe.
fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_generates_and_then_reuses_the_token() {
        let (jar, token) = ensure_csrf_token(CookieJar::new(), false);
        assert!(!token.is_empty());

        // Segunda chamada com a mesma jar devolve o mesmo token (não rotaciona
        // a cada página, senão duas abas abertas invalidariam uma à outra).
        let (_, again) = ensure_csrf_token(jar, false);
        assert_eq!(token, again);
    }

    #[test]
    fn verify_accepts_matching_token_and_rejects_the_rest() {
        let (jar, token) = ensure_csrf_token(CookieJar::new(), false);

        assert!(verify_csrf(&jar, &token).is_ok());
        assert!(matches!(
            verify_csrf(&jar, "forged-token"),
            Err(AppError::CsrfMismatch)
        ));
        // Sem cookie nenhum, qualquer token é rejeitado.
        assert!(matches!(
            verify_csrf(&CookieJar::new(), &token),
            Err(AppError::CsrfMismatch)
        ));
    }

    #[test]
    fn tokens_are_unpredictable() {
        let (_, a) = ensure_csrf_token(CookieJar::new(), false);
        let (_, b) = ensure_csrf_token(CookieJar::new(), false);
        assert_ne!(a, b);
    }
}
