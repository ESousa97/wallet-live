use askama::Template;
use axum::Router;
use axum::extract::{Form, Query, State};
use axum::response::{Html, Redirect};
use axum::routing::get;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::csrf::{ensure_csrf_token, verify_csrf};
use crate::auth::session::{
    REFRESH_COOKIE, RefreshToken, access_cookie, hash_token, refresh_cookie, session_expiry,
};
use crate::auth::user::{TOKEN_COOKIE, UnauthenticatedUser, User};
use crate::config::Config;
use crate::error::AppError;
use crate::models::{Asset, Holding, Transaction, WalletSummary};
use crate::quotes::sync_market_quotes;
use crate::repository::Repository;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/login", get(login_page).post(login))
        .route("/register", get(register_page).post(register))
        .route("/logout", get(logout))
        .route("/assets", get(assets_page))
        .route("/deposit", get(deposit_page).post(deposit))
        .route("/buy", get(buy_page).post(buy_asset))
        .route("/sell", get(sell_page).post(sell_asset))
        .route("/quotes/sync", get(assets_page).post(sync_quotes))
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    is_register: bool,
    csrf_token: String,
}

/// Toda página com formulário garante um token CSRF na jar e o embute num campo
/// oculto; os POSTs correspondentes conferem os dois (ver `auth::csrf`).
#[instrument(skip_all)]
async fn login_page(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Html<String>), AppError> {
    let (jar, csrf_token) = ensure_csrf_token(jar, state.config.cookie_secure);
    let page = LoginPage {
        is_register: false,
        csrf_token,
    };
    Ok((jar, Html(page.render()?)))
}

#[instrument(skip_all)]
async fn register_page(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Html<String>), AppError> {
    let (jar, csrf_token) = ensure_csrf_token(jar, state.config.cookie_secure);
    let page = LoginPage {
        is_register: true,
        csrf_token,
    };
    Ok((jar, Html(page.render()?)))
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
    csrf_token: String,
}

#[instrument(skip_all)]
async fn login(
    State(state): State<AppState>,
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), AppError> {
    verify_csrf(&jar, &form.csrf_token)?;

    // Lockout ANTES de conferir a senha: durante o bloqueio nem a senha certa
    // passa, então força bruta não extrai sinal nenhum das tentativas.
    state.login_throttle.ensure_allowed(&form.username).await?;

    let user = match UnauthenticatedUser::new(form.username.clone(), form.password)
        .authenticate(&repository)
        .await
    {
        Ok(user) => {
            state.login_throttle.record_success(&form.username).await;
            user
        }
        // Só falhas de credencial alimentam o contador — sondagem de username
        // (404) e senha errada (401) contam igual; erro de banco não.
        Err(error @ (AppError::InvalidCredentials | AppError::UserDoesNotExist)) => {
            state.login_throttle.record_failure(&form.username).await;
            return Err(error);
        }
        Err(error) => return Err(error),
    };

    let jar = start_session(jar, &user, &repository, &state.config).await?;
    Ok((jar, Redirect::to("/")))
}

#[instrument(skip_all)]
async fn register(
    State(state): State<AppState>,
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), AppError> {
    verify_csrf(&jar, &form.csrf_token)?;

    let user = UnauthenticatedUser::new(form.username, form.password)
        .register(&repository)
        .await?;

    let jar = start_session(jar, &user, &repository, &state.config).await?;
    Ok((jar, Redirect::to("/")))
}

/// Emite o par de cookies da sessão: o JWT de acesso (curto) e o refresh token
/// (longo), cujo registro vai para a tabela `sessions` — é ele que permite
/// renovar o acesso sem novo login e revogar a sessão no logout.
async fn start_session(
    jar: CookieJar,
    user: &User,
    repository: &Repository,
    config: &Config,
) -> Result<CookieJar, AppError> {
    let refresh = RefreshToken::generate();
    repository
        .create_session(user.id(), &refresh.hash(), session_expiry(config))
        .await?;

    Ok(jar
        .add(access_cookie(user, config)?)
        .add(refresh_cookie(&refresh, config)))
}

/// Logout com revogação REAL: além de remover os cookies do navegador, mata a
/// sessão no servidor — o refresh token para de funcionar em qualquer cópia.
#[instrument(skip_all)]
async fn logout(repository: Repository, jar: CookieJar) -> (CookieJar, Redirect) {
    if let Some(refresh) = jar.get(REFRESH_COOKIE) {
        // Falha ao revogar (banco fora etc.) não impede o logout local; a
        // sessão ainda expira sozinha pelo expires_at.
        let _ = repository
            .revoke_session(&hash_token(refresh.value()))
            .await;
    }

    (
        jar.remove(Cookie::build(TOKEN_COOKIE).path("/").build())
            .remove(Cookie::build(REFRESH_COOKIE).path("/").build()),
        Redirect::to("/login"),
    )
}

#[instrument(skip_all)]
async fn index(maybe_user: Option<User>) -> Redirect {
    match maybe_user {
        Some(_) => Redirect::to("/assets"),
        None => Redirect::to("/login"),
    }
}

#[derive(Template)]
#[template(path = "assets.html")]
struct AssetsPage {
    holdings: Vec<Holding>,
    available_assets: Vec<Asset>,
    transactions: Vec<Transaction>,
    summary: WalletSummary,
    user: User,
    action: WalletAction,
    csrf_token: String,
    page: u32,
    has_prev: bool,
    has_next: bool,
}

enum WalletAction {
    None,
    Deposit,
    Buy,
    Sell,
}

/// Transações por página do extrato.
const TRANSACTIONS_PAGE_SIZE: i64 = 25;

#[derive(Deserialize)]
struct PageQuery {
    page: Option<u32>,
}

#[instrument(skip_all)]
async fn assets_page(
    State(state): State<AppState>,
    user: User,
    repository: Repository,
    jar: CookieJar,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(state, user, repository, jar, WalletAction::None, &query).await
}

#[instrument(skip_all)]
async fn deposit_page(
    State(state): State<AppState>,
    user: User,
    repository: Repository,
    jar: CookieJar,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(state, user, repository, jar, WalletAction::Deposit, &query).await
}

#[instrument(skip_all)]
async fn buy_page(
    State(state): State<AppState>,
    user: User,
    repository: Repository,
    jar: CookieJar,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(state, user, repository, jar, WalletAction::Buy, &query).await
}

#[instrument(skip_all)]
async fn sell_page(
    State(state): State<AppState>,
    user: User,
    repository: Repository,
    jar: CookieJar,
    Query(query): Query<PageQuery>,
) -> Result<(CookieJar, Html<String>), AppError> {
    render_wallet(state, user, repository, jar, WalletAction::Sell, &query).await
}

async fn render_wallet(
    state: AppState,
    user: User,
    repository: Repository,
    jar: CookieJar,
    action: WalletAction,
    query: &PageQuery,
) -> Result<(CookieJar, Html<String>), AppError> {
    let (jar, csrf_token) = ensure_csrf_token(jar, state.config.cookie_secure);

    // Página 1-based; qualquer valor inválido cai na primeira.
    let page = query.page.unwrap_or(1).max(1);
    let offset = i64::from(page - 1) * TRANSACTIONS_PAGE_SIZE;

    let (summary, holdings, available_assets, transactions, total_transactions) = tokio::try_join!(
        repository.wallet_summary(user.id()),
        repository.list_holdings(user.id()),
        repository.list_assets(),
        repository.list_transactions(user.id(), TRANSACTIONS_PAGE_SIZE, offset),
        repository.count_transactions(user.id())
    )?;

    let has_next = (offset + transactions.len() as i64) < total_transactions;

    let page = AssetsPage {
        holdings,
        available_assets,
        transactions,
        summary,
        user,
        action,
        csrf_token,
        page,
        has_prev: page > 1,
        has_next,
    };
    Ok((jar, Html(page.render()?)))
}

#[derive(Deserialize)]
struct AmountForm {
    amount: Decimal,
    csrf_token: String,
}

#[instrument(skip_all)]
async fn deposit(
    user: User,
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<AmountForm>,
) -> Result<Redirect, AppError> {
    verify_csrf(&jar, &form.csrf_token)?;
    repository.deposit(user.id(), form.amount).await?;
    Ok(Redirect::to("/assets"))
}

#[derive(Deserialize)]
struct TradeAssetForm {
    asset_id: i64,
    quantity: Decimal,
    csrf_token: String,
}

#[instrument(skip_all)]
async fn buy_asset(
    user: User,
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<TradeAssetForm>,
) -> Result<Redirect, AppError> {
    verify_csrf(&jar, &form.csrf_token)?;
    repository
        .buy_asset(user.id(), form.asset_id, form.quantity)
        .await?;
    Ok(Redirect::to("/assets"))
}

#[instrument(skip_all)]
async fn sell_asset(
    user: User,
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<TradeAssetForm>,
) -> Result<Redirect, AppError> {
    verify_csrf(&jar, &form.csrf_token)?;
    repository
        .sell_asset(user.id(), form.asset_id, form.quantity)
        .await?;
    Ok(Redirect::to("/assets"))
}

/// O formulário de sincronizar cotações não tem campo de dado nenhum, mas ainda
/// é um POST que muda estado — então também carrega (e valida) o token CSRF.
#[derive(Deserialize)]
struct SyncQuotesForm {
    csrf_token: String,
}

#[instrument(skip_all)]
async fn sync_quotes(
    _user: User,
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<SyncQuotesForm>,
) -> Result<Redirect, AppError> {
    verify_csrf(&jar, &form.csrf_token)?;
    sync_market_quotes(&repository).await?;
    Ok(Redirect::to("/assets"))
}

pub mod filters {
    use askama::Values;
    use rust_decimal::Decimal;
    use time::OffsetDateTime;

    #[askama::filter_fn]
    pub fn human_datetime(value: &OffsetDateTime, _: &dyn Values) -> askama::Result<String> {
        let format = time::format_description::parse("[year]-[month]-[day] [hour]:[minute]")
            .map_err(askama::Error::custom)?;

        value.format(&format).map_err(askama::Error::custom)
    }

    #[askama::filter_fn]
    pub fn money(value: &Decimal, _: &dyn Values) -> askama::Result<String> {
        let raw = format!("{:.2}", value.abs());
        let (integer, cents) = raw
            .split_once('.')
            .ok_or_else(|| askama::Error::custom("invalid decimal format"))?;

        let mut grouped = String::new();
        for (index, character) in integer.chars().rev().enumerate() {
            if index > 0 && index % 3 == 0 {
                grouped.push('.');
            }
            grouped.push(character);
        }
        let integer = grouped.chars().rev().collect::<String>();
        let sign = if value.is_sign_negative() { "- " } else { "" };

        Ok(format!("{sign}R$ {integer},{cents}"))
    }

    #[askama::filter_fn]
    pub fn quantity(value: &Decimal, _: &dyn Values) -> askama::Result<String> {
        Ok(value.normalize().to_string())
    }

    #[askama::filter_fn]
    pub fn nonnegative(value: &Decimal, _: &dyn Values) -> askama::Result<bool> {
        Ok(*value >= Decimal::ZERO)
    }

    #[askama::filter_fn]
    pub fn transaction_kind(value: &str, _: &dyn Values) -> askama::Result<&'static str> {
        Ok(match value {
            "deposit" => "deposito",
            "buy" => "compra",
            "sell" => "venda",
            _ => "movimentacao",
        })
    }
}
