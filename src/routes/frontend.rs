use askama::Template;
use axum::extract::Form;
use axum::response::{Html, Redirect};
use axum::routing::get;
use axum::Router;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::user::{UnauthenticatedUser, User, TOKEN_COOKIE};
use crate::error::AppError;
use crate::models::{Asset, OwnedAsset};
use crate::repository::Repository;

/// Rotas do front-end (SSR). Diferente da API, devolvem HTML em vez de JSON.
/// Usam o mesmo `AppState`, de onde sai o `Repository`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/login", get(login_page).post(login))
        .route("/logout", get(logout))
        .route("/assets", get(assets_page).post(purchase_asset))
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

/// Encerra a sessão: remove o cookie `token` e volta para o login. Não precisa
/// validar nada antes — remover um cookie que não existe não é um erro.
#[instrument(skip_all)]
async fn logout(jar: CookieJar) -> (CookieJar, Redirect) {
    (jar.remove(TOKEN_COOKIE), Redirect::to("/login"))
}

/// Agora o index é só um roteador: usa `Option<User>` para decidir para onde
/// mandar o visitante, sem nunca devolver conteúdo próprio. Com sessão válida vai
/// para `/assets`; sem ela, para `/login`. A própria `/assets` não repete essa
/// checagem — ela já exige `User`.
#[instrument(skip_all)]
async fn index(maybe_user: Option<User>) -> Redirect {
    match maybe_user {
        Some(_) => Redirect::to("/assets"),
        None => Redirect::to("/login"),
    }
}

/// Template da tela de ativos: o que o usuário possui (com lucro/prejuízo e
/// histórico de compras) e os ativos disponíveis no sistema, usados para
/// popular o formulário de compra.
#[derive(Template)]
#[template(path = "assets.html")]
struct AssetsPage {
    owned_assets: Vec<OwnedAsset>,
    available_assets: Vec<Asset>,
    user: User,
    // Resumo do portfólio, agregado sobre todas as posições. Calculado no
    // handler: template não é lugar de regra de negócio.
    total_value: f64,
    total_invested: f64,
    total_delta: f64,
}

/// Tela principal: ativos que o usuário possui (com quanto ele tem hoje,
/// lucro/prejuízo total e histórico de compras) e os ativos disponíveis no
/// sistema. As duas consultas não dependem uma da outra, então `try_join!` as
/// executa concorrentemente em vez de aguardar uma depois da outra.
#[instrument(skip_all)]
async fn assets_page(user: User, repository: Repository) -> Result<Html<String>, AppError> {
    let (owned_assets, available_assets) = tokio::try_join!(
        repository.list_owned_assets(user.id()),
        repository.list_assets()
    )?;

    // Valor atual do portfólio (preço atual × quantidade, somado por posição) e
    // lucro/prejuízo total. O investido sai por diferença: atual - delta.
    let total_value: f64 = owned_assets
        .iter()
        .map(|asset| asset.unit_value * asset.quantity_owned)
        .sum();
    let total_delta: f64 = owned_assets.iter().map(|asset| asset.value_delta).sum();
    let total_invested = total_value - total_delta;

    let html = AssetsPage {
        owned_assets,
        available_assets,
        user,
        total_value,
        total_invested,
        total_delta,
    }
    .render()?;

    Ok(Html(html))
}

/// Campos do formulário de compra — têm que bater com os `name` dos inputs no
/// HTML (o `<select>` de ativos manda o `id`, não o nome).
#[derive(Deserialize)]
struct PurchaseAssetForm {
    asset_id: i64,
    quantity: f64,
    unit_value: f64,
}

/// Registra a compra de um ativo. O usuário vem do cookie (JWT), nunca do
/// formulário — assim ninguém pode submeter uma compra em nome de outra pessoa.
#[instrument(skip_all)]
async fn purchase_asset(
    user: User,
    repository: Repository,
    Form(form): Form<PurchaseAssetForm>,
) -> Result<Redirect, AppError> {
    repository
        .add_owned_asset(user.id(), form.asset_id, form.quantity, form.unit_value)
        .await?;

    Ok(Redirect::to("/assets"))
}

/// Filtros customizados usados pelo template `assets.html`. O Askama procura um
/// módulo `filters` no mesmo arquivo onde o template (`AssetsPage`) é definido.
pub mod filters {
    use askama::Values;
    use time::OffsetDateTime;

    /// Formata um timestamp como `AAAA-MM-DD HH:MM`. O erro só pode acontecer se
    /// a descrição do formato em si estiver malformada — nunca por causa de
    /// `value`, que sempre vem de uma data válida do banco.
    #[askama::filter_fn]
    pub fn human_datetime(value: &OffsetDateTime, _: &dyn Values) -> askama::Result<String> {
        let format = time::format_description::parse("[year]-[month]-[day] [hour]:[minute]")
            .map_err(askama::Error::custom)?;

        value.format(&format).map_err(askama::Error::custom)
    }

    /// Formata um valor monetário com duas casas decimais fixas. Os modelos usam
    /// `f64` (escolha didática do curso), e o Display padrão exporia o ruído de
    /// arredondamento de float na tela — `-9.999999999999998` em vez de `-10.00`.
    #[askama::filter_fn]
    pub fn money(value: &f64, _: &dyn Values) -> askama::Result<String> {
        Ok(format!("{value:.2}"))
    }
}
