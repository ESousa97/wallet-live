use askama::Template;
use axum::extract::Form;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::user::{UnauthenticatedUser, User, TOKEN_COOKIE};
use crate::error::AppError;
use crate::repository::Repository;

/// Rotas do front-end (SSR). Diferente da API, devolvem HTML em vez de JSON.
/// Usam o mesmo `AppState`, de onde sai o `Repository`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/login", get(login_page).post(login))
}

/// Template da tela de login. Não carrega dado nenhum — o Askama lê o arquivo de
/// `templates/login.html` e checa o template em tempo de compilação.
#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage;

/// Serve o formulário de login (GET). `Html` só ajusta o content-type da resposta.
#[instrument(skip_all)]
async fn login_page() -> Result<Html<String>, AppError> {
    let html = LoginPage.render()?;
    Ok(Html(html))
}

/// Campos do formulário — têm que bater com os `name` dos inputs no HTML.
#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

/// Processa o envio do formulário (POST). Para simplificar, a mesma tela faz
/// login E cadastro: se o usuário existe, autentica; se não, registra. Em vez de
/// devolver HTML, agora grava o JWT num cookie e redireciona para o index — assim
/// a sessão sobrevive a um F5.
#[instrument(skip_all)]
async fn login(
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), AppError> {
    let unauth_user = UnauthenticatedUser::new(form.username, form.password);

    let user = match unauth_user.authenticate(&repository).await {
        Ok(user) => user,
        // Usuário inexistente é o gatilho para cadastrá-lo agora.
        Err(AppError::UserDoesNotExist) => unauth_user.register(&repository).await?,
        // Qualquer outro erro (senha errada, banco) sobe normalmente.
        Err(error) => return Err(error),
    };

    // O token assinado vai num cookie `http_only` (inacessível ao JS do
    // navegador). Devolver a `jar` na resposta emite o header Set-Cookie.
    let token = user.auth_token()?;
    let cookie = Cookie::build((TOKEN_COOKIE, token)).http_only(true).build();

    Ok((jar.add(cookie), Redirect::to("/")))
}

/// Tela principal (por enquanto só cumprimenta o usuário). Usa `Option<User>`:
/// se não há sessão válida (sem token, ou token inválido/expirado), redireciona
/// para o login em vez de devolver um erro.
#[instrument(skip_all)]
async fn index(maybe_user: Option<User>) -> Response {
    match maybe_user {
        Some(user) => Html(format!("Hello {}", user.username())).into_response(),
        None => Redirect::to("/login").into_response(),
    }
}
