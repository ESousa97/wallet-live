use askama::Template;
use axum::Router;
use axum::extract::Form;
use axum::response::{Html, Redirect};
use axum::routing::get;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::instrument;

use crate::app::AppState;
use crate::auth::user::{TOKEN_COOKIE, UnauthenticatedUser, User};
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
}

#[instrument(skip_all)]
async fn login_page() -> Result<Html<String>, AppError> {
    Ok(Html(LoginPage { is_register: false }.render()?))
}

#[instrument(skip_all)]
async fn register_page() -> Result<Html<String>, AppError> {
    Ok(Html(LoginPage { is_register: true }.render()?))
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[instrument(skip_all)]
async fn login(
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), AppError> {
    let user = UnauthenticatedUser::new(form.username, form.password)
        .authenticate(&repository)
        .await?;

    Ok((jar.add(session_cookie(user)?), Redirect::to("/")))
}

#[instrument(skip_all)]
async fn register(
    repository: Repository,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> Result<(CookieJar, Redirect), AppError> {
    let user = UnauthenticatedUser::new(form.username, form.password)
        .register(&repository)
        .await?;

    Ok((jar.add(session_cookie(user)?), Redirect::to("/")))
}

#[instrument(skip_all)]
async fn logout(jar: CookieJar) -> (CookieJar, Redirect) {
    (
        jar.remove(Cookie::build(TOKEN_COOKIE).path("/").build()),
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
}

enum WalletAction {
    None,
    Deposit,
    Buy,
    Sell,
}

#[instrument(skip_all)]
async fn assets_page(user: User, repository: Repository) -> Result<Html<String>, AppError> {
    render_wallet(user, repository, WalletAction::None).await
}

#[instrument(skip_all)]
async fn deposit_page(user: User, repository: Repository) -> Result<Html<String>, AppError> {
    render_wallet(user, repository, WalletAction::Deposit).await
}

#[instrument(skip_all)]
async fn buy_page(user: User, repository: Repository) -> Result<Html<String>, AppError> {
    render_wallet(user, repository, WalletAction::Buy).await
}

#[instrument(skip_all)]
async fn sell_page(user: User, repository: Repository) -> Result<Html<String>, AppError> {
    render_wallet(user, repository, WalletAction::Sell).await
}

async fn render_wallet(
    user: User,
    repository: Repository,
    action: WalletAction,
) -> Result<Html<String>, AppError> {
    let (summary, holdings, available_assets, transactions) = tokio::try_join!(
        repository.wallet_summary(user.id()),
        repository.list_holdings(user.id()),
        repository.list_assets(),
        repository.list_transactions(user.id())
    )?;

    Ok(Html(
        AssetsPage {
            holdings,
            available_assets,
            transactions,
            summary,
            user,
            action,
        }
        .render()?,
    ))
}

#[derive(Deserialize)]
struct AmountForm {
    amount: Decimal,
}

#[instrument(skip_all)]
async fn deposit(
    user: User,
    repository: Repository,
    Form(form): Form<AmountForm>,
) -> Result<Redirect, AppError> {
    repository.deposit(user.id(), form.amount).await?;
    Ok(Redirect::to("/assets"))
}

#[derive(Deserialize)]
struct TradeAssetForm {
    asset_id: i64,
    quantity: Decimal,
}

#[instrument(skip_all)]
async fn buy_asset(
    user: User,
    repository: Repository,
    Form(form): Form<TradeAssetForm>,
) -> Result<Redirect, AppError> {
    repository
        .buy_asset(user.id(), form.asset_id, form.quantity)
        .await?;
    Ok(Redirect::to("/assets"))
}

#[instrument(skip_all)]
async fn sell_asset(
    user: User,
    repository: Repository,
    Form(form): Form<TradeAssetForm>,
) -> Result<Redirect, AppError> {
    repository
        .sell_asset(user.id(), form.asset_id, form.quantity)
        .await?;
    Ok(Redirect::to("/assets"))
}

#[instrument(skip_all)]
async fn sync_quotes(_user: User, repository: Repository) -> Result<Redirect, AppError> {
    sync_market_quotes(&repository).await?;
    Ok(Redirect::to("/assets"))
}

fn session_cookie(user: User) -> Result<Cookie<'static>, AppError> {
    let secure = std::env::var("COOKIE_SECURE")
        .map(|value| value == "true")
        .unwrap_or(false);

    Ok(Cookie::build((TOKEN_COOKIE, user.auth_token()?))
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(secure)
        .path("/")
        .build())
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
