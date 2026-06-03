use askama::Template;
use axum::extract::Form;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::user::UnauthenticatedUser;
use crate::error::AppError;
use crate::repository::Repository;

/// Rotas do front-end (SSR). Diferente da API, devolvem HTML em vez de JSON.
/// Usam o mesmo `AppState`, de onde sai o `Repository`.
pub fn router() -> Router<AppState> {
    Router::new().route("/login", get(login_page).post(login))
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
/// login E cadastro: se o usuário existe, autentica; se não, registra.
#[instrument(skip_all)]
async fn login(
    repository: Repository,
    Form(form): Form<LoginForm>,
) -> Result<Html<String>, AppError> {
    let unauth_user = UnauthenticatedUser::new(form.username, form.password);

    let user = match unauth_user.authenticate(&repository).await {
        Ok(user) => user,
        // Usuário inexistente é o gatilho para cadastrá-lo agora.
        Err(AppError::UserDoesNotExist) => unauth_user.register(&repository).await?,
        // Qualquer outro erro (senha errada, banco) sobe normalmente.
        Err(error) => return Err(error),
    };

    // Por enquanto só confirmamos a autorização ecoando o nome. A sessão (manter
    // o login após F5) chega na próxima aula.
    Ok(Html(user.username().to_string()))
}
